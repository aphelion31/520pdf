use crate::http::{
    auth::{self, AuthConfig},
    tokens::{self, CorsMatch, TokenAuthError},
};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use chrono::{SecondsFormat, Utc};
use lopdf::Document;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};

const CORS_ALLOW_METHODS: &str = "GET, POST, PUT, DELETE, OPTIONS";
const CORS_ALLOW_HEADERS: &str = "Authorization, Content-Type, X-API-Key, X-Job-Id, Accept";
const CORS_EXPOSE_HEADERS: &str = "Content-Disposition, x-520pdf-token-id, x-compress-status, x-compress-dpi, x-compress-quality, x-job-id";
const MAX_PROGRESS_JOBS: usize = 256;

struct JobProgress {
    percent: u8,
    stage: String,
    done: bool,
    updated_at: Instant,
}

type ProgressMap = Arc<Mutex<HashMap<String, JobProgress>>>;

struct App {
    jobs: Arc<Semaphore>,
    auth: Arc<AuthConfig>,
    progress: ProgressMap,
}

fn valid_job_id(raw: &str) -> bool {
    let s = raw.trim();
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn sanitize_job_id(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("x-job-id")?.to_str().ok()?.trim();
    valid_job_id(raw).then(|| raw.to_owned())
}

fn set_job_progress(map: &ProgressMap, job_id: &str, percent: u8, stage: &str, done: bool) {
    let Ok(mut guard) = map.lock() else {
        return;
    };
    let now = Instant::now();
    if guard.len() >= MAX_PROGRESS_JOBS {
        guard.retain(|_, v| now.duration_since(v.updated_at) < Duration::from_secs(300));
    }
    let next_pct = guard
        .get(job_id)
        .map_or(percent, |prev| prev.percent.max(percent))
        .min(100);
    guard.insert(
        job_id.to_owned(),
        JobProgress {
            percent: next_pct,
            stage: stage.to_owned(),
            done,
            updated_at: now,
        },
    );
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}

fn apply_cors_headers(
    headers: &mut HeaderMap,
    auth: &AuthConfig,
    req_origin: Option<&str>,
    path: &str,
    preflight_req_headers: Option<&str>,
) {
    let origins = auth.store().cors_origins();
    let allow_all = origins.iter().any(|o| o == "*");
    let matched = req_origin.map(|o| (o, auth.store().match_cors_origin(o)));

    match matched {
        Some((origin, CorsMatch::Explicit)) => {
            if let Ok(v) = HeaderValue::from_str(origin) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );
            }
            headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        }
        Some((_, CorsMatch::Wildcard)) => {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        }
        Some((_, CorsMatch::Denied)) => {
            headers.insert(header::VARY, HeaderValue::from_static("Origin"));
            return;
        }
        None => {
            if allow_all && !path.starts_with("/api/auth/") {
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    HeaderValue::from_static("*"),
                );
            } else {
                return;
            }
        }
    }

    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(CORS_ALLOW_METHODS),
    );
    let allow_headers_val = preflight_req_headers
        .map(str::trim)
        .filter(|h| {
            !h.is_empty()
                && h.len() <= 512
                && h.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b',' | b' '))
        })
        .and_then(|h| HeaderValue::from_str(h).ok())
        .unwrap_or_else(|| HeaderValue::from_static(CORS_ALLOW_HEADERS));
    headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, allow_headers_val);
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(CORS_EXPOSE_HEADERS),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
}

async fn api_cors_middleware(
    State(auth): State<Arc<AuthConfig>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_owned();
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }
    let req_origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let preflight_headers = req
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    if req.method() == Method::OPTIONS {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NO_CONTENT;
        apply_cors_headers(
            res.headers_mut(),
            &auth,
            req_origin.as_deref(),
            &path,
            preflight_headers.as_deref(),
        );
        return res;
    }

    let mut res = next.run(req).await;
    apply_cors_headers(res.headers_mut(), &auth, req_origin.as_deref(), &path, None);
    res
}

pub fn build_router(web_dir: PathBuf) -> Router {
    let auth = Arc::new(AuthConfig::from_env().expect("invalid AUTH_* configuration"));
    build_router_with_auth(web_dir, auth)
}

pub fn build_router_with_auth(web_dir: PathBuf, auth: Arc<AuthConfig>) -> Router {
    let cors_auth = auth.clone();
    let state = Arc::new(App {
        jobs: Arc::new(Semaphore::new(2)),
        auth: auth.clone(),
        progress: Arc::new(Mutex::new(HashMap::new())),
    });
    let index_html = web_dir.join("index.html");
    let spa = ServeDir::new(&web_dir).fallback(ServeFile::new(index_html));

    let auth_router = Router::new()
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/logout", post(auth::logout))
        .route(
            "/api/tokens",
            get(tokens::list_tokens_handler).post(tokens::create_token_handler),
        )
        .route("/api/tokens/{id}", delete(tokens::revoke_token_handler))
        .route(
            "/api/tokens/{id}/reveal",
            post(tokens::reveal_token_handler),
        )
        .route(
            "/api/cors",
            get(tokens::get_cors_handler).put(tokens::update_cors_handler),
        )
        .with_state(auth);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ready", get(|| async { "ok" }))
        .route("/api/status", get(api_status))
        .route("/api/tools", get(|| async { Json(catalog()) }))
        .route("/api/openapi.json", get(api_openapi))
        .route("/api/docs/search", get(api_docs_search))
        .route("/api/jobs/{id}", get(job_progress_handler))
        .route("/api/tools/{tool}", post(process))
        .route("/api/v1/tools/{tool}", post(process))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(state)
        .merge(auth_router)
        .fallback_service(spa)
        .layer(middleware::from_fn_with_state(
            cors_auth,
            api_cors_middleware,
        ))
        .layer(CompressionLayer::new())
}

async fn job_progress_handler(State(app): State<Arc<App>>, Path(id): Path<String>) -> Response {
    if !valid_job_id(&id) {
        return error(StatusCode::BAD_REQUEST, "Invalid job id");
    }
    let (percent, stage, done) = app
        .progress
        .lock()
        .ok()
        .and_then(|g| {
            g.get(id.trim())
                .map(|p| (p.percent, p.stage.clone(), p.done))
        })
        .unwrap_or((0, "Starting…".to_string(), false));
    let mut res = (
        StatusCode::OK,
        Json(json!({
            "job_id": id.trim(),
            "percent": percent,
            "stage": stage,
            "done": done,
        })),
    )
        .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn api_status() -> Response {
    let body = serde_json::to_string(&json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }))
    .unwrap_or_else(|_| "{\"status\":\"ok\"}".into());
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

async fn shutdown_signal() {
    let terminate = async {
        #[cfg(unix)]
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
            return;
        }
        std::future::pending::<()>().await;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate => {},
    }
}

pub fn main() -> std::process::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let web_dir = PathBuf::from(
            std::env::var("WEB_DIR")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "web/build/client".to_string()),
        );
        let app = build_router(web_dir);
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);
        let bind = std::env::var("BIND")
            .or_else(|_| std::env::var("BIND_ADDR"))
            .unwrap_or_else(|_| format!("127.0.0.1:{port}"));
        let listener = match tokio::net::TcpListener::bind(&bind).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("bind {bind}: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        println!("520pdf listening on {bind}");
        match axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
        {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                std::process::ExitCode::FAILURE
            }
        }
    })
}

const MIN_COMPRESS_KB: usize = 250;

fn sanitize_upload_stem(raw_name: Option<&str>, tool: &str) -> String {
    let raw_stem = raw_name
        .and_then(|n| n.rsplit(['/', '\\']).next())
        .and_then(|n| std::path::Path::new(n).file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let mut stem = String::with_capacity(raw_stem.len().min(80));
    let mut prev_underscore = false;
    for c in raw_stem.chars() {
        let ch = if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
            c
        } else {
            '_'
        };
        if ch == '_' {
            if !prev_underscore && !stem.is_empty() {
                stem.push('_');
            }
            prev_underscore = true;
        } else {
            prev_underscore = false;
            stem.push(ch);
        }
        if stem.len() >= 80 {
            break;
        }
    }
    let trimmed = stem.trim_matches(['_', '.']).to_string();
    if !trimmed.is_empty() {
        trimmed
    } else if matches!(tool, "resume-builder" | "latex-resume") {
        "resume".to_string()
    } else if tool == "equation-to-image" {
        "equation".to_string()
    } else {
        "document".to_string()
    }
}

fn format_download_filename(
    upload_name: Option<&str>,
    tool: &str,
    output_path: &std::path::Path,
) -> String {
    let stem = sanitize_upload_stem(upload_name, tool);
    let ext = output_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_matches('.'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or("pdf");
    format!("{stem}_520pdf_{tool}.{ext}").replace(['"', '\r', '\n'], "")
}

fn has_inline_payload(tool: &str, options: &HashMap<String, String>) -> bool {
    match tool {
        "resume-builder" => options
            .get("resume_json")
            .is_some_and(|v| !v.trim().is_empty()),
        "latex-resume" => options.get("latex").is_some_and(|v| !v.trim().is_empty()),
        "equation-to-image" => options
            .get("expression")
            .is_some_and(|v| !v.trim().is_empty()),
        "html-to-pdf" => options.get("html").is_some_and(|v| !v.trim().is_empty()),
        _ => false,
    }
}

fn resolve_compress_target_kb(
    input: &std::path::Path,
    options: &HashMap<String, String>,
) -> Result<usize, String> {
    if let Some(raw) = options.get("target_kb").filter(|s| !s.trim().is_empty()) {
        let kb = raw
            .parse::<usize>()
            .map_err(|_| "Invalid target KB".to_string())?;
        if !(MIN_COMPRESS_KB..=100_000).contains(&kb) {
            return Err("Target must be 250–100000 KB".into());
        }
        return Ok(kb);
    }
    if let Some(preset) = options
        .get("quality_preset")
        .map(|s| s.trim().to_ascii_lowercase())
    {
        let in_kb = (std::fs::metadata(input).map(|m| m.len()).unwrap_or(250_000) / 1000)
            .max(MIN_COMPRESS_KB as u64);
        let ratio = match preset.as_str() {
            "low" => 75,
            "medium" => 45,
            "high" => 25,
            _ => return Err("quality_preset must be low, medium or high".into()),
        };
        if preset == "high" {
            return Ok(MIN_COMPRESS_KB);
        }
        return Ok(((in_kb * ratio as u64) / 100).clamp(MIN_COMPRESS_KB as u64, 100_000) as usize);
    }
    Ok(MIN_COMPRESS_KB)
}

async fn process(
    State(app): State<Arc<App>>,
    Path(tool): Path<String>,
    headers: HeaderMap,
    form: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Response {
    let job_id = sanitize_job_id(&headers);
    let mut used_token_id: Option<String> = None;
    if let Some(raw_token) = auth::extract_api_token(&headers) {
        match app.auth.verify_api_token(&raw_token, &tool) {
            Ok(identity) => used_token_id = Some(identity.id),
            Err(TokenAuthError::Unauthorized(msg)) => {
                return error(StatusCode::UNAUTHORIZED, msg);
            }
            Err(TokenAuthError::Forbidden(msg)) => {
                return error(StatusCode::FORBIDDEN, msg);
            }
        }
    } else {
        if !app.auth.is_authorized(&headers) {
            return error(StatusCode::UNAUTHORIZED, "Authentication required");
        }
        // Browser requests validate against configured CORS origins when cross-origin.
        if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            let host = headers
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let is_same_origin =
                origin == format!("https://{host}") || origin == format!("http://{host}");
            if !is_same_origin {
                let has_session_cookie = headers
                    .get(header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|c| c.contains("520pdf_session="));
                let cors_match = app.auth.store().match_cors_origin(origin);
                let allowed = if has_session_cookie {
                    cors_match == CorsMatch::Explicit
                } else {
                    cors_match != CorsMatch::Denied
                };
                if !allowed {
                    return error(StatusCode::FORBIDDEN, "Cross-origin upload rejected");
                }
            }
        }
    }
    if !catalog()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["id"] == tool)
    {
        return error(StatusCode::NOT_FOUND, "Unknown tool");
    }
    if let Some(ref jid) = job_id {
        set_job_progress(&app.progress, jid, 18, "Receiving upload…", false);
    }
    let mut form = match form {
        Ok(f) => f,
        Err(e) => {
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Failed", true);
            }
            return error(e.status(), "Expected multipart/form-data upload");
        }
    };
    let permit = match app.jobs.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Busy", true);
            }
            return error(
                StatusCode::TOO_MANY_REQUESTS,
                "Two jobs are already running. Try again shortly.",
            );
        }
    };
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Temporary storage unavailable",
            );
        }
    };
    let mut inputs = Vec::<PathBuf>::new();
    let mut first_upload_name: Option<String> = None;
    let mut options = HashMap::new();
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return error(e.status(), "Invalid or oversized upload"),
        };
        let name = field.name().unwrap_or("").to_owned();
        if let Some(orig_name) = field.file_name().map(str::to_owned) {
            if inputs.len() >= 30 {
                return error(StatusCode::BAD_REQUEST, "Maximum 30 files per job");
            }
            if first_upload_name.is_none() && !orig_name.trim().is_empty() {
                first_upload_name = Some(orig_name.clone());
            }
            let ext = std::path::Path::new(&orig_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("bin")
                .to_ascii_lowercase();
            if !ext.bytes().all(|b| b.is_ascii_alphanumeric()) || ext.len() > 8 {
                return error(StatusCode::BAD_REQUEST, "Invalid file extension");
            }
            let path = dir.path().join(format!("upload-{}.{}", inputs.len(), ext));
            let data = match field.bytes().await {
                Ok(b) => b,
                Err(e) => return error(e.status(), "Invalid or oversized upload"),
            };
            if let Err(e) = std::fs::write(&path, data) {
                return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
            }
            inputs.push(path);
        } else {
            if options.len() >= 30 {
                return error(StatusCode::BAD_REQUEST, "Too many options");
            }
            let max_opt_len = match name.as_str() {
                "annotations" => 8_000_000,
                "html" | "photo" => 2_000_000,
                _ => 200_000,
            };
            match field.text().await {
                Ok(v) if v.len() <= max_opt_len => {
                    options.insert(name, v);
                }
                _ => return error(StatusCode::BAD_REQUEST, "Invalid option"),
            }
        }
    }
    if let Some(fields) = options.remove("fields") {
        let values: HashMap<String, String> = match serde_json::from_str(&fields) {
            Ok(v) => v,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "Field values must be a JSON object of strings",
                );
            }
        };
        for (key, value) in values {
            options.insert(format!("field.{key}"), value);
        }
    }
    if inputs.is_empty() && !has_inline_payload(&tool, &options) {
        if let Some(ref jid) = job_id {
            set_job_progress(&app.progress, jid, 100, "Missing input", true);
        }
        return error(StatusCode::UNPROCESSABLE_ENTITY, "Select at least one file");
    }
    if let Some(ref jid) = job_id {
        set_job_progress(&app.progress, jid, 25, "Queued for processing", false);
    }
    let progress_map = app.progress.clone();
    let job_id_worker = job_id.clone();
    let job = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let report = |pct: u8, stage: &str| {
            if let Some(ref jid) = job_id_worker {
                set_job_progress(&progress_map, jid, pct, stage, false);
            }
        };
        let mut compression = None;
        let visual_protect = tool == "protect"
            && (options.get("mode").map(String::as_str) == Some("visual")
                || options.get("password").is_none_or(|p| p.trim().is_empty()));
        let result = if tool == "compress" {
            if inputs.len() != 1 {
                return Err("Compression requires exactly one PDF".into());
            }
            let kb = resolve_compress_target_kb(&inputs[0], &options)?;
            let max_dpi = match options
                .get("dpi")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                Some(raw) => {
                    let d = raw
                        .parse::<u32>()
                        .map_err(|_| "dpi must be an integer between 36 and 300".to_string())?;
                    if !(36..=300).contains(&d) {
                        return Err("dpi must be between 36 and 300".into());
                    }
                    Some(d)
                }
                None => None,
            };
            let jpeg_quality = match options
                .get("quality")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                Some(raw) => {
                    let q = raw
                        .parse::<u8>()
                        .map_err(|_| "quality must be an integer between 15 and 95".to_string())?;
                    if !(15..=95).contains(&q) {
                        return Err("quality must be between 15 and 95".into());
                    }
                    Some(q)
                }
                None => None,
            };
            let (dpi, quality, mut bytes) = crate::compress_with_options_progress(
                &inputs[0],
                crate::CompressOptions {
                    target_bytes: kb * 1000,
                    max_dpi,
                    jpeg_quality,
                    force_recompress: max_dpi.is_some() || jpeg_quality.is_some(),
                },
                |pct, stage| report(pct, stage),
            )?;
            if options.get("remove_metadata").map(String::as_str) == Some("true")
                && let Ok(mut doc) = Document::load_mem(&bytes)
            {
                report(94, "Stripping metadata");
                crate::tools::strip_metadata(&mut doc);
                let mut stripped = Vec::new();
                if doc.save_to(&mut stripped).is_ok() {
                    bytes = stripped;
                }
            }
            report(98, "Finalizing PDF");
            compression = Some((
                if bytes.len() <= kb * 1000 {
                    "OK"
                } else {
                    "OVER_TARGET"
                },
                dpi,
                quality,
            ));
            let p = dir.path().join("520pdf-compressed.pdf");
            std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
            Ok((p, "application/pdf".into()))
        } else {
            report(32, &format!("Processing {tool}…"));
            let out = if visual_protect
                || matches!(
                    tool.as_str(),
                    "merge"
                        | "split"
                        | "organize"
                        | "rotate"
                        | "crop"
                        | "watermark"
                        | "page-numbers"
                        | "forms"
                        | "sign"
                        | "image-to-pdf"
                        | "edit-pdf"
                ) {
                crate::tools::run(&tool, &inputs, &options, dir.path())
            } else if matches!(
                tool.as_str(),
                "pdf-to-word"
                    | "pdf-to-powerpoint"
                    | "pdf-to-excel"
                    | "word-to-pdf"
                    | "powerpoint-to-pdf"
                    | "excel-to-pdf"
            ) {
                crate::features::office::run(&tool, &inputs, &options, dir.path())
            } else if matches!(tool.as_str(), "pdf-to-image" | "compress-image") {
                crate::features::new_media::run(&tool, &inputs, &options, dir.path())
            } else if matches!(tool.as_str(), "resume-builder" | "latex-resume") {
                crate::features::resume::run(&tool, &inputs, &options, dir.path())
            } else if tool == "equation-to-image" {
                crate::features::equation::run(&tool, &inputs, &options, dir.path())
            } else if matches!(tool.as_str(), "summarize" | "translate") {
                crate::ai_tools::run(&tool, &inputs, &options, dir.path())
            } else {
                crate::conversions::run(&tool, &inputs, &options, dir.path())
            };
            if out.is_ok() {
                report(94, "Preparing output…");
            }
            out
        };
        let (path, mime) = result?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let filename = format_download_filename(first_upload_name.as_deref(), &tool, &path);
        Ok::<_, String>((bytes, mime, filename, compression))
    });
    match tokio::time::timeout(std::time::Duration::from_secs(180), job).await {
        Ok(Ok(Ok((bytes, mime, name, compression)))) => {
            let mut res = Response::new(Body::from(bytes));
            let h = res.headers_mut();
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Complete", true);
                if let Ok(val) = HeaderValue::from_str(jid) {
                    h.insert("x-job-id", val);
                }
            }
            if let Some(ref tid) = used_token_id
                && let Ok(val) = HeaderValue::from_str(tid)
            {
                h.insert("x-520pdf-token-id", val);
            }
            if let Some((status, dpi, quality)) = compression {
                h.insert("x-compress-status", HeaderValue::from_str(status).unwrap());
                h.insert(
                    "x-compress-dpi",
                    HeaderValue::from_str(&dpi.to_string()).unwrap(),
                );
                h.insert(
                    "x-compress-quality",
                    HeaderValue::from_str(&quality.to_string()).unwrap(),
                );
            }
            if let Ok(v) = mime.parse() {
                h.insert(header::CONTENT_TYPE, v);
            }
            if let Ok(v) = format!("attachment; filename=\"{name}\"").parse() {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            h.insert(
                "x-content-type-options",
                HeaderValue::from_static("nosniff"),
            );
            res
        }
        Ok(Ok(Err(e))) => {
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Failed", true);
            }
            error(StatusCode::UNPROCESSABLE_ENTITY, e)
        }
        Ok(Err(_)) => {
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Failed", true);
            }
            error(StatusCode::INTERNAL_SERVER_ERROR, "Processing failed")
        }
        Err(_) => {
            if let Some(ref jid) = job_id {
                set_job_progress(&app.progress, jid, 100, "Timed out", true);
            }
            error(
                StatusCode::GATEWAY_TIMEOUT,
                "Job exceeded 180 seconds. Worker slot stays reserved until processing stops.",
            )
        }
    }
}

fn field(name: &str, label: &str, kind: &str, value: &str) -> Value {
    json!({"name":name,"label":label,"type":kind,"value":value})
}

fn select_field(name: &str, label: &str, value: &str, options: &[(&str, &str)]) -> Value {
    let opts: Vec<Value> = options
        .iter()
        .map(|(v, l)| json!({"value": v, "label": l}))
        .collect();
    json!({"name": name, "label": label, "type": "select", "value": value, "options": opts})
}

fn with_help(mut v: Value, help: &str) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.insert("help".into(), json!(help));
    }
    v
}

fn tool_route(id: &str) -> String {
    match id {
        "merge" => "/merge-pdfs".into(),
        "split" => "/split-pdf".into(),
        "compress" => "/compress-pdf".into(),
        "compress-image" => "/compress-image".into(),
        "pdf-to-image" => "/pdf-to-image".into(),
        "image-to-pdf" => "/image-to-pdf".into(),
        "watermark" => "/watermark-pdf".into(),
        "protect" => "/lock-pdf".into(),
        "sign" => "/sign-pdf".into(),
        "edit-pdf" => "/edit-pdf".into(),
        "equation-to-image" => "/equation-to-image".into(),
        "latex-resume" => "/latex-resume".into(),
        "resume-builder" => "/resume-builder".into(),
        other => format!("/tools/{other}"),
    }
}

const TOOL_ROWS: &[(&str, &str, &str, &str, &str, bool)] = &[
    (
        "merge",
        "Merge PDFs",
        "Combine multiple PDF files into one document in upload order.",
        "Organize",
        ".pdf",
        true,
    ),
    (
        "split",
        "Split PDF",
        "Extract specific pages or burst a PDF into separate files.",
        "Organize",
        ".pdf",
        false,
    ),
    (
        "compress",
        "Compress PDF",
        "Reduce PDF file size using presets or a custom KB target.",
        "Optimize",
        ".pdf",
        false,
    ),
    (
        "compress-image",
        "Compress Image",
        "Shrink JPG, PNG, or WebP files with quality, width, and KB targets.",
        "Optimize",
        ".png,.jpg,.jpeg,.webp",
        false,
    ),
    (
        "pdf-to-image",
        "PDF to Image",
        "Export PDF pages as PNG, JPG, or WebP images or extract embedded images.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "image-to-pdf",
        "Images to PDF",
        "Turn JPG, PNG, or WebP photos into a PDF with page sizing.",
        "Convert",
        ".jpg,.jpeg,.png,.webp",
        true,
    ),
    (
        "watermark",
        "Watermark PDF",
        "Add diagonal text or image watermarks to document pages.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "protect",
        "Protect PDF",
        "Add confidentiality stamps or lock your PDF with password encryption.",
        "Security",
        ".pdf",
        false,
    ),
    (
        "sign",
        "Sign PDF",
        "Draw, upload, or type a visible signature and place it on pages.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "equation-to-image",
        "Equation to Image",
        "Render LaTeX math equations into crisp transparent PNG or JPG images.",
        "Create",
        ".txt",
        false,
    ),
    (
        "latex-resume",
        "LaTeX Resume",
        "Write your resume in LaTeX with templates and export directly to PDF.",
        "Create",
        ".tex,.txt",
        false,
    ),
    (
        "resume-builder",
        "Resume Builder",
        "Build a structured resume visually or via JSON and export to PDF.",
        "Create",
        ".json",
        false,
    ),
    (
        "organize",
        "Organize PDF",
        "Reorder, duplicate, or remove pages visually.",
        "Organize",
        ".pdf",
        false,
    ),
    (
        "rotate",
        "Rotate PDF",
        "Rotate one or multiple PDFs 90, 180, or 270 degrees.",
        "Organize",
        ".pdf",
        true,
    ),
    (
        "edit-pdf",
        "Edit PDF",
        "Add text, highlights, shapes, freehand drawings, and images onto PDF pages.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "crop",
        "Crop PDF",
        "Adjust visible page crop boxes or margins.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "page-numbers",
        "Page numbers",
        "Number the pages of a PDF with custom formatting.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "forms",
        "PDF forms",
        "Discover and fill interactive PDF form fields.",
        "Edit",
        ".pdf",
        false,
    ),
    (
        "office-to-pdf",
        "Office to PDF",
        "Convert Word, PowerPoint, and spreadsheet files to PDF.",
        "Convert",
        ".doc,.docx,.ppt,.pptx,.xls,.xlsx,.odt",
        false,
    ),
    (
        "pdf-to-word",
        "PDF to Word",
        "Convert PDF documents into editable Word (.docx) files.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "pdf-to-powerpoint",
        "PDF to PowerPoint",
        "Turn PDF pages into PowerPoint (.pptx) slides (visual or editable text).",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "pdf-to-excel",
        "PDF to Excel",
        "Extract PDF tables and text into Excel (.xlsx) spreadsheets.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "word-to-pdf",
        "Word to PDF",
        "Convert Word (.docx, .doc, .odt, .rtf) documents into PDF.",
        "Convert",
        ".doc,.docx,.odt,.rtf",
        false,
    ),
    (
        "powerpoint-to-pdf",
        "PowerPoint to PDF",
        "Convert PowerPoint (.pptx, .ppt, .odp) slideshows into PDF.",
        "Convert",
        ".ppt,.pptx,.odp",
        false,
    ),
    (
        "excel-to-pdf",
        "Excel to PDF",
        "Convert Excel (.xlsx, .xls, .ods, .csv) spreadsheets into PDF.",
        "Convert",
        ".xls,.xlsx,.ods,.csv",
        false,
    ),
    (
        "ocr-pdf",
        "OCR PDF",
        "Make scanned PDFs searchable and selectable with local OCR.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "html-to-pdf",
        "HTML to PDF",
        "Render uploaded HTML without external network resource access.",
        "Convert",
        ".html,.htm",
        false,
    ),
    (
        "compare",
        "Compare PDF",
        "Compare two PDFs side by side for text and visual differences.",
        "Edit",
        ".pdf",
        true,
    ),
    (
        "pdf-to-markdown",
        "PDF to Markdown",
        "Extract document text and structure as Markdown.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "unlock",
        "Unlock PDF",
        "Remove encryption using the known document password.",
        "Security",
        ".pdf",
        false,
    ),
    (
        "pdfa",
        "PDF to PDF/A",
        "Convert to archival PDF/A-2b and verify compliance.",
        "Convert",
        ".pdf",
        false,
    ),
    (
        "scan-to-pdf",
        "Scan to PDF",
        "Run OCR on scanned PDFs or images into searchable PDF.",
        "Convert",
        ".pdf,.jpg,.jpeg,.png",
        false,
    ),
    (
        "repair",
        "Repair PDF",
        "Fix damaged PDF files and recover unreadable pages.",
        "Optimize",
        ".pdf",
        false,
    ),
    (
        "redact",
        "Redact PDF",
        "Permanently black out sensitive text and regions from a PDF.",
        "Security",
        ".pdf",
        false,
    ),
    (
        "summarize",
        "AI summarize",
        "Summarize extracted text with your configured provider.",
        "AI",
        ".pdf",
        false,
    ),
    (
        "translate",
        "Translate PDF",
        "Translate page-by-page to text or PDF with your provider.",
        "AI",
        ".pdf",
        false,
    ),
];

pub fn all_tool_ids() -> Vec<&'static str> {
    TOOL_ROWS.iter().map(|r| r.0).collect()
}

pub fn catalog() -> Value {
    let tools: Vec<Value> = TOOL_ROWS
        .iter()
        .copied()
        .map(|(id, name, desc, cat, accept, multiple)| {
            let fields = match id {
                "pdf-to-image" => vec![
                    with_help(
                        select_field(
                            "mode",
                            "Export style",
                            "pages",
                            &[
                                ("pages", "Convert every page"),
                                ("extract", "Extract embedded photos"),
                            ],
                        ),
                        "Convert every page renders full PDF pages as images. Extract embedded photos pulls out original pictures inside the PDF.",
                    ),
                    with_help(
                        field("pages", "Pages", "text", ""),
                        "Leave blank for all pages, or enter specific pages like 1,3-5.",
                    ),
                    with_help(
                        select_field(
                            "dpi",
                            "Image resolution",
                            "150",
                            &[
                                ("72", "72 DPI (Screen)"),
                                ("150", "150 DPI (Standard)"),
                                ("200", "200 DPI (High)"),
                                ("300", "300 DPI (Print)"),
                            ],
                        ),
                        "Higher DPI gives sharper images with larger file size.",
                    ),
                    select_field(
                        "format",
                        "Image format",
                        "png",
                        &[("png", "PNG"), ("jpeg", "JPEG"), ("webp", "WebP")],
                    ),
                ],
                "compress-image" => vec![
                    with_help(
                        field("target_kb", "Target size (KB)", "number", "250"),
                        "Desired maximum image size in KB.",
                    ),
                    with_help(
                        field("quality", "Image quality", "number", "80"),
                        "Quality between 1 (smallest) and 100 (sharpest).",
                    ),
                    with_help(
                        field("max_width", "Max width (px)", "number", "1920"),
                        "Wider images are scaled down proportionally to fit this width.",
                    ),
                    select_field(
                        "output_format",
                        "Format",
                        "auto",
                        &[
                            ("auto", "Keep source format"),
                            ("jpeg", "JPEG"),
                            ("webp", "WebP"),
                            ("png", "PNG"),
                        ],
                    ),
                ],
                "equation-to-image" => vec![
                    with_help(
                        field("expression", "Math expression", "textarea", "E = mc^2"),
                        "Write standard LaTeX math such as E = mc^2 or \\frac{a}{b}.",
                    ),
                    field("font_size", "Font size", "number", "32"),
                ],
                "latex-resume" => vec![with_help(
                    field("latex", "LaTeX source", "textarea", ""),
                    "Paste LaTeX source code for your resume.",
                )],
                "resume-builder" => vec![with_help(
                    field("resume_json", "Resume content", "textarea", ""),
                    "Structured resume JSON data used to generate the resume PDF.",
                )],
                "compress" => vec![
                    json!({
                        "name": "target_kb",
                        "label": "Target size (KB)",
                        "type": "number",
                        "value": "250",
                        "min": 250,
                        "max": 100000,
                        "step": 1,
                        "help": "Desired maximum file size in KB. Lowest supported target is 250 KB."
                    }),
                    with_help(
                        select_field(
                            "dpi",
                            "Image resolution",
                            "",
                            &[
                                ("", "Auto (Recommended)"),
                                ("300", "300 DPI"),
                                ("200", "200 DPI"),
                                ("150", "150 DPI"),
                                ("120", "120 DPI"),
                                ("110", "110 DPI"),
                                ("96", "96 DPI"),
                                ("72", "72 DPI"),
                                ("48", "48 DPI"),
                            ],
                        ),
                        "Caps how sharp embedded photos stay. Lower DPI shrinks files further; Auto tunes it automatically.",
                    ),
                    json!({
                        "name": "quality",
                        "label": "Image quality",
                        "type": "number",
                        "value": "",
                        "min": 15,
                        "max": 95,
                        "step": 1,
                        "help": "Photo quality from 15 (smallest) to 95 (sharpest). Leave blank for automatic tuning."
                    }),
                    with_help(
                        select_field(
                            "quality_preset",
                            "Compression level",
                            "high",
                            &[
                                ("high", "Maximum shrink (Recommended)"),
                                ("medium", "Balanced"),
                                ("low", "High visual quality"),
                            ],
                        ),
                        "Choose how aggressively file size is reduced versus visual detail.",
                    ),
                    with_help(
                        field("remove_metadata", "Remove document info", "checkbox", "true"),
                        "Clears hidden author, title, and software properties from the PDF.",
                    ),
                ],
                "split" => vec![
                    with_help(
                        field("pages", "Pages", "text", "1"),
                        "Pages or ranges to keep, for example: 1,3-5,2.",
                    ),
                    with_help(
                        select_field(
                            "mode",
                            "Split style",
                            "extract",
                            &[
                                ("extract", "Extract selected pages"),
                                ("all", "One file per page (ZIP)"),
                                ("ranges", "Separate page groups"),
                            ],
                        ),
                        "Extract picks matching pages into one PDF. One file per page exports every page separately. Separate page groups splits by ranges below.",
                    ),
                    with_help(
                        field("ranges", "Page groups", "text", ""),
                        "Separate groups with semicolons, for example: 1-2;3-4.",
                    ),
                ],
                "organize" => vec![with_help(
                    field("pages", "Page order", "text", "1"),
                    "Target page sequence (duplicates allowed), for example: 1,3,1.",
                )],
                "rotate" => vec![with_help(
                    select_field(
                        "angle",
                        "Rotation",
                        "90",
                        &[
                            ("90", "90° Clockwise"),
                            ("180", "180° Flip"),
                            ("270", "270° Counter-clockwise"),
                        ],
                    ),
                    "Direction to rotate every page in the uploaded PDF(s).",
                )],
                "crop" => vec![
                    with_help(
                        field("x", "Left offset", "number", "20"),
                        "Distance in points from the left edge of the page.",
                    ),
                    with_help(
                        field("y", "Bottom offset", "number", "20"),
                        "Distance in points from the bottom edge of the page.",
                    ),
                    with_help(
                        field("width", "Width", "number", "550"),
                        "Width of the cropped visible area in points (A4 is 595 pt wide).",
                    ),
                    with_help(
                        field("height", "Height", "number", "750"),
                        "Height of the cropped visible area in points (A4 is 842 pt tall).",
                    ),
                ],
                "image-to-pdf" => vec![
                    select_field(
                        "page_size",
                        "Page size",
                        "a4",
                        &[
                            ("a4", "A4"),
                            ("letter", "US Letter"),
                            ("fit", "Fit to image"),
                        ],
                    ),
                    select_field(
                        "orientation",
                        "Orientation",
                        "portrait",
                        &[("portrait", "Portrait"), ("landscape", "Landscape")],
                    ),
                    with_help(
                        field("margin", "Page margin", "number", "24"),
                        "White space around each image in points (24 pt is about 8.5 mm).",
                    ),
                ],
                "watermark" | "sign" => {
                    let mut list = vec![
                        field(
                            "text",
                            if id == "sign" {
                                "Signature text"
                            } else {
                                "Watermark text"
                            },
                            "text",
                            "",
                        ),
                        with_help(
                            field("x", "Horizontal position", "number", "36"),
                            "Distance from the left edge in points.",
                        ),
                        with_help(
                            field("y", "Vertical position", "number", "72"),
                            "Distance from the bottom edge in points.",
                        ),
                        field("width", "Width", "number", "160"),
                        field("height", "Height", "number", "60"),
                        field("font_size", "Text size", "number", "32"),
                        field("rotation", "Rotation angle", "number", "0"),
                        with_help(
                            field("opacity", "Opacity", "number", "0.5"),
                            "Transparency from 0 (invisible) to 1 (solid).",
                        ),
                    ];
                    if id == "watermark" {
                        list.push(field("color", "Text color", "text", "#E63228"));
                        list.push(with_help(
                            field("count", "Repeat count", "number", "1"),
                            "Total stamps per page (1 to 64).",
                        ));
                        list.push(field("cols", "Columns", "number", "1"));
                        list.push(field("rows", "Rows", "number", "1"));
                    }
                    list
                }
                "page-numbers" => vec![
                    field("start", "Start number", "number", "1"),
                    with_help(
                        field("format", "Number format", "text", "Page {n} of {total}"),
                        "Use {n} for current page number and {total} for total pages.",
                    ),
                    select_field(
                        "position",
                        "Position",
                        "center",
                        &[
                            ("left", "Bottom Left"),
                            ("center", "Bottom Center"),
                            ("right", "Bottom Right"),
                        ],
                    ),
                    with_help(
                        field("y", "Bottom margin", "number", "24"),
                        "Distance from the bottom edge in points.",
                    ),
                ],
                "protect" => vec![
                    with_help(
                        select_field(
                            "mode",
                            "Protection type",
                            "visual",
                            &[
                                ("visual", "Confidentiality stamp"),
                                ("password", "Password lock"),
                            ],
                        ),
                        "Confidentiality stamp adds a visible overlay and clears hidden metadata. Password lock encrypts the PDF so it needs a password to open.",
                    ),
                    field(
                        "watermark_text",
                        "Stamp text",
                        "text",
                        "CONFIDENTIAL",
                    ),
                    with_help(
                        field("remove_metadata", "Remove document info", "checkbox", "true"),
                        "Clears hidden author, title, and software properties.",
                    ),
                    with_help(
                        field("password", "Open password", "password", ""),
                        "Password required to open the encrypted PDF (used in Password lock mode).",
                    ),
                    with_help(
                        field("owner_password", "Permissions password", "password", ""),
                        "Separate admin password to change print/edit limits (must differ from Open password).",
                    ),
                    select_field(
                        "print",
                        "Printing",
                        "full",
                        &[
                            ("full", "Full resolution"),
                            ("low", "Low resolution"),
                            ("none", "Disabled"),
                        ],
                    ),
                    select_field(
                        "modify",
                        "Changes allowed",
                        "all",
                        &[
                            ("all", "All modifications"),
                            ("annotate", "Annotations"),
                            ("form", "Form filling"),
                            ("assembly", "Page assembly"),
                            ("none", "Disabled"),
                        ],
                    ),
                    select_field(
                        "extract",
                        "Allow text copying",
                        "y",
                        &[("y", "Yes"), ("n", "No")],
                    ),
                ],
                "unlock" => vec![field("password", "PDF password", "password", "")],
                "forms" => vec![
                    with_help(
                        select_field(
                            "action",
                            "Action",
                            "fill",
                            &[
                                ("fill", "Fill form fields"),
                                ("discover", "List field names"),
                            ],
                        ),
                        "List field names inspects fillable boxes inside the PDF; Fill form fields saves your values into them.",
                    ),
                    with_help(
                        field("fields", "Field values", "textarea", "{}"),
                        "JSON map of field names to values, for example {\"FullName\":\"Nadia\"}.",
                    ),
                ],
                "redact" => vec![with_help(
                    field("rectangles", "Areas to black out", "textarea", "[]"),
                    "JSON list of boxes to permanently cover: [{\"page\":1,\"x\":72,\"y\":700,\"width\":160,\"height\":24}].",
                )],
                "scan-to-pdf" => vec![
                    with_help(
                        select_field(
                            "language",
                            "Document language",
                            "eng",
                            &[
                                ("eng", "English"),
                                ("ind", "Indonesian"),
                                ("vie", "Vietnamese"),
                                ("ind+eng", "Indonesian + English"),
                            ],
                        ),
                        "Select the language(s) in your scan so text recognition stays accurate.",
                    ),
                    with_help(
                        field("deskew", "Straighten tilted pages", "checkbox", "true"),
                        "Automatically rotates slightly crooked scans upright before reading text.",
                    ),
                ],
                "ocr-pdf" => vec![
                    with_help(
                        select_field(
                            "language",
                            "Document language",
                            "eng",
                            &[
                                ("eng", "English"),
                                ("ind", "Indonesian"),
                                ("vie", "Vietnamese"),
                                ("ind+eng", "Indonesian + English"),
                            ],
                        ),
                        "Select the language(s) in your document for accurate text recognition.",
                    ),
                    with_help(
                        field("deskew", "Straighten tilted pages", "checkbox", "true"),
                        "Automatically rotates slightly crooked scans upright.",
                    ),
                    with_help(
                        field("force_ocr", "Re-scan all pages", "checkbox", "false"),
                        "Run text recognition on every page even if some selectable text already exists.",
                    ),
                ],
                "pdf-to-word" => vec![
                    with_help(
                        select_field(
                            "mode",
                            "Layout style",
                            "auto",
                            &[
                                ("auto", "Smart editable (Auto OCR)"),
                                ("ocr", "Scanned document (Force OCR)"),
                                ("exact", "Exact visual match"),
                                ("flow", "Simple text & tables"),
                            ],
                        ),
                        "Smart editable keeps digital layouts editable and automatically runs text recognition on scanned pages. Scanned document runs full text and table recognition on every page. Exact visual match locks elements in place to look identical to the PDF. Simple text & tables creates lightweight reflowable text.",
                    ),
                    with_help(
                        select_field(
                            "ocr_language",
                            "OCR language",
                            "ind+eng",
                            &[
                                ("ind+eng", "Indonesian + English"),
                                ("eng", "English"),
                                ("ind", "Indonesian"),
                            ],
                        ),
                        "Language used when reading text from scanned or image-based pages.",
                    ),
                ],
                "pdf-to-powerpoint" => vec![with_help(
                    select_field(
                        "mode",
                        "Slide style",
                        "visual",
                        &[
                            ("visual", "Visual slides"),
                            ("editable", "Editable text"),
                        ],
                    ),
                    "Visual slides mirror the PDF page look; Editable text puts text inside movable PowerPoint boxes.",
                )],
                "pdf-to-excel" => vec![with_help(
                    select_field(
                        "merge_sheets",
                        "Worksheet layout",
                        "false",
                        &[
                            ("false", "One worksheet per PDF page"),
                            ("true", "Merge all pages into one sheet"),
                        ],
                    ),
                    "Choose whether each PDF page becomes its own Excel tab or combines onto one sheet.",
                )],
                "html-to-pdf" => vec![with_help(
                    field("html", "Paste HTML", "textarea", ""),
                    "Optional HTML snippet to turn into a PDF without uploading a file. External links stay disabled for privacy.",
                )],
                "edit-pdf" => vec![with_help(
                    field("annotations", "Annotations", "textarea", "[]"),
                    "JSON list of text notes, highlights, shapes, or drawings to add.",
                )],
                "summarize" | "translate" => vec![
                    field("target_language", "Translation language", "text", "English"),
                    field("summary_language", "Summary language", "text", "English"),
                    select_field(
                        "summary_length",
                        "Summary length",
                        "medium",
                        &[("short", "Short"), ("medium", "Medium"), ("long", "Long")],
                    ),
                    select_field(
                        "output_format",
                        "Output format",
                        "pdf",
                        &[("pdf", "PDF"), ("txt", "Plain Text (.txt)")],
                    ),
                    field(
                        "bilingual",
                        "Include original text with translation",
                        "checkbox",
                        "false",
                    ),
                    with_help(
                        field("ocr", "Recognize text in scans", "checkbox", "false"),
                        "Enable if your PDF only contains scanned page images with no selectable text.",
                    ),
                    with_help(
                        select_field(
                            "ocr_language",
                            "Scan language",
                            "eng",
                            &[
                                ("eng", "English"),
                                ("ind", "Indonesian"),
                                ("ind+eng", "Indonesian + English"),
                            ],
                        ),
                        "Language used when recognizing text from scanned pages.",
                    ),
                    field(
                        "consent",
                        "Allow sending extracted document text to the AI provider",
                        "checkbox",
                        "false",
                    ),
                ],
                _ => vec![],
            };
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "category": cat,
                "route": tool_route(id),
                "accept": accept,
                "multiple": multiple,
                "fields": fields,
            })
        })
        .collect();
    json!({ "tools": tools })
}

async fn api_openapi() -> Json<Value> {
    let cat = catalog();
    let tools = cat["tools"].as_array().cloned().unwrap_or_default();
    let mut paths = serde_json::Map::new();

    paths.insert(
        "/api/auth/register".into(),
        json!({
            "post": {
                "summary": "Register an Argon2id account to manage API keys",
                "tags": ["Auth"],
                "requestBody": {
                    "required": true,
                    "content": { "application/json": { "schema": { "type": "object", "required": ["username", "password"], "properties": { "username": { "type": "string" }, "password": { "type": "string" } } } } }
                },
                "responses": { "201": { "description": "Account created & session cookie issued" } }
            }
        }),
    );
    paths.insert(
        "/api/auth/login".into(),
        json!({
            "post": {
                "summary": "Sign in with Argon2id credentials",
                "tags": ["Auth"],
                "responses": { "200": { "description": "Authenticated session cookie issued" } }
            }
        }),
    );
    paths.insert(
        "/api/tokens".into(),
        json!({
            "get": {
                "summary": "List API token metadata for signed-in user",
                "tags": ["API Keys"],
                "responses": { "200": { "description": "Token list (hashes never exposed)" }, "401": { "description": "Login required" } }
            },
            "post": {
                "summary": "Create a scoped API token (secret returned once)",
                "tags": ["API Keys"],
                "requestBody": {
                    "required": true,
                    "content": { "application/json": { "schema": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" }, "scopes": { "type": "array", "items": { "type": "string" } }, "expires_in_days": { "type": "integer" } } } } }
                },
                "responses": { "201": { "description": "Token minted" }, "401": { "description": "Login required" } }
            }
        }),
    );
    paths.insert(
        "/api/tokens/{id}".into(),
        json!({
            "delete": {
                "summary": "Revoke an owned API token",
                "tags": ["API Keys"],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                "responses": { "200": { "description": "Revoked" } }
            }
        }),
    );
    paths.insert(
        "/api/tokens/{id}/reveal".into(),
        json!({
            "post": {
                "summary": "Reveal an owned API token after re-verifying login password",
                "tags": ["API Keys"],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                "requestBody": {
                    "required": true,
                    "content": { "application/json": { "schema": { "type": "object", "required": ["password"], "properties": { "password": { "type": "string" } } } } }
                },
                "responses": {
                    "200": { "description": "Decrypted API token returned (Cache-Control: no-store)" },
                    "400": { "description": "Missing password or token revoked" },
                    "401": { "description": "Session missing or incorrect password" },
                    "404": { "description": "Token not found" },
                    "409": { "description": "Legacy token created before reveal support" }
                }
            }
        }),
    );
    paths.insert(
        "/api/cors".into(),
        json!({
            "get": {
                "summary": "List configured API CORS allowed origins (default *)",
                "tags": ["CORS"],
                "responses": { "200": { "description": "Active CORS origins list" }, "401": { "description": "Login required" } }
            },
            "put": {
                "summary": "Update API CORS allowed origins list",
                "tags": ["CORS"],
                "requestBody": {
                    "required": true,
                    "content": { "application/json": { "schema": { "type": "object", "required": ["origins"], "properties": { "origins": { "type": "array", "items": { "type": "string" } } } } } }
                },
                "responses": { "200": { "description": "CORS origins updated" }, "400": { "description": "Invalid origin" }, "401": { "description": "Login required" } }
            }
        }),
    );
    paths.insert(
        "/api/jobs/{id}".into(),
        json!({
            "get": {
                "summary": "Poll live percentage progress and stage for a running tool job",
                "tags": ["Tools"],
                "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                "responses": { "200": { "description": "Progress percentage and current stage" }, "400": { "description": "Invalid job id" } }
            }
        }),
    );

    for t in &tools {
        let id = t["id"].as_str().unwrap_or("");
        let name = t["name"].as_str().unwrap_or(id);
        let desc = t["description"].as_str().unwrap_or("");
        let cat_name = t["category"].as_str().unwrap_or("Tools");
        let mut props = serde_json::Map::new();
        props.insert(
            "files".into(),
            json!({ "type": "string", "format": "binary", "description": format!("Input files ({})", t["accept"].as_str().unwrap_or("")) }),
        );
        if let Some(fields) = t["fields"].as_array() {
            for f in fields {
                if let Some(fname) = f["name"].as_str() {
                    props.insert(
                        fname.into(),
                        json!({
                            "type": "string",
                            "description": f["label"].as_str().unwrap_or(fname),
                            "default": f["value"],
                        }),
                    );
                }
            }
        }
        let op = json!({
            "post": {
                "summary": name,
                "description": desc,
                "tags": [cat_name],
                "security": [{ "bearerAuth": [] }, { "apiKeyAuth": [] }],
                "requestBody": {
                    "required": true,
                    "content": { "multipart/form-data": { "schema": { "type": "object", "properties": props } } }
                },
                "responses": {
                    "200": { "description": "Processed output file" },
                    "401": { "description": "Invalid/missing credentials or API key" },
                    "403": { "description": "API key lacks tool scope" }
                }
            }
        });
        paths.insert(format!("/api/v1/tools/{id}"), op);
    }

    Json(json!({
        "openapi": "3.1.0",
        "info": {
            "title": "520pdf API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Single-binary Rust PDF, Office, OCR & media workbench API"
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "520pdf_k_..." },
                "apiKeyAuth": { "type": "apiKey", "in": "header", "name": "X-API-Key" }
            }
        },
        "paths": paths
    }))
}

async fn api_docs_search(Query(params): Query<HashMap<String, String>>) -> Json<Value> {
    let q = params
        .get("query")
        .or_else(|| params.get("q"))
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default();

    let mut docs_index: Vec<(String, String, String, String)> = vec![
        (
            "doc:index".into(),
            "/docs".into(),
            "Introduction to 520pdf API — endpoints /api/v1/tools/{tool}, limits, OpenAPI 3.1".into(),
            "Documentation".into(),
        ),
        (
            "doc:auth".into(),
            "/docs/authentication".into(),
            "Authentication & Argon2id Sessions — login, registration, REQUIRE_API_KEY, Bearer".into(),
            "Documentation".into(),
        ),
        (
            "doc:tokens".into(),
            "/docs/api-tokens".into(),
            "API Tokens — create, scope, expiry, revoke 520pdf_k_ keys via /api-keys".into(),
            "Documentation".into(),
        ),
        (
            "doc:compress".into(),
            "/docs/compress-pdf".into(),
            "Compress PDF Customization — target_kb >= 250, dpi 36-300, quality 15-95, preset, strip metadata".into(),
            "Guides".into(),
        ),
        (
            "doc:tools".into(),
            "/docs/tools-reference".into(),
            "36-Tool API Parameter Reference — Office, OCR, Edit PDF, Watermark grid, Split, Merge".into(),
            "Reference".into(),
        ),
        (
            "doc:examples".into(),
            "/docs/examples".into(),
            "Code Examples — curl, TypeScript fetch, Python requests with API keys".into(),
            "Guides".into(),
        ),
    ];

    for t in catalog()["tools"].as_array().into_iter().flatten() {
        let id = t["id"].as_str().unwrap_or("");
        let name = t["name"].as_str().unwrap_or(id);
        let desc = t["description"].as_str().unwrap_or("");
        let cat = t["category"].as_str().unwrap_or("Tools");
        let route = t["route"].as_str().unwrap_or("/");
        docs_index.push((
            format!("tool:{id}"),
            route.to_owned(),
            format!("{name} (/api/v1/tools/{id}) — {desc}"),
            cat.to_owned(),
        ));
    }

    let results: Vec<Value> = docs_index
        .into_iter()
        .filter(|(_, url, content, cat)| {
            q.is_empty()
                || content.to_ascii_lowercase().contains(&q)
                || url.to_ascii_lowercase().contains(&q)
                || cat.to_ascii_lowercase().contains(&q)
        })
        .take(20)
        .map(|(id, url, content, cat)| {
            json!({
                "id": id,
                "url": url,
                "type": "page",
                "content": content,
                "breadcrumbs": [cat],
            })
        })
        .collect();

    Json(json!(results))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_36_tools_and_routes() {
        let cat = catalog();
        let tools = cat["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 36);
        assert!(
            tools
                .iter()
                .any(|t| t["id"] == "latex-resume" && t["route"] == "/latex-resume")
        );
        assert!(
            tools
                .iter()
                .any(|t| t["id"] == "merge" && t["route"] == "/merge-pdfs")
        );
        assert!(
            tools
                .iter()
                .any(|t| t["id"] == "edit-pdf" && t["route"] == "/edit-pdf")
        );
        assert!(
            tools
                .iter()
                .any(|t| t["id"] == "pdf-to-word" && t["route"] == "/tools/pdf-to-word")
        );
    }

    #[tokio::test]
    async fn api_status_catalog_and_spa_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("index.html"),
            "<!doctype html><html><body>520pdf</body></html>",
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router =
            build_router_with_auth(tmp.path().to_path_buf(), Arc::new(AuthConfig::disabled()));
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        let base = format!("http://{addr}");
        let status_body: Value = reqwest::Client::new()
            .get(format!("{base}/api/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(status_body["status"], "ok");
        assert!(status_body["timestamp"].as_str().unwrap().ends_with('Z'));

        let tools_body: Value = reqwest::Client::new()
            .get(format!("{base}/api/tools"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(tools_body["tools"].as_array().unwrap().len(), 36);

        let spa_html = reqwest::Client::new()
            .get(format!("{base}/resume-builder"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(spa_html.contains("520pdf"));

        let me_body: Value = reqwest::Client::new()
            .get(format!("{base}/api/auth/me"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(me_body["auth_enabled"], false);
        assert_eq!(me_body["algorithm"], "argon2id");

        handle.abort();
    }

    #[test]
    fn filename_and_min_compress_target_enforce_conventions() {
        assert_eq!(
            format_download_filename(
                Some("NPWP.pdf"),
                "compress",
                std::path::Path::new("out.pdf")
            ),
            "NPWP_520pdf_compress.pdf"
        );
        assert_eq!(
            format_download_filename(
                Some("C:\\Users\\test\\My NPWP (2026).pdf"),
                "merge",
                std::path::Path::new("merged.pdf")
            ),
            "My_NPWP_2026_520pdf_merge.pdf"
        );
        assert_eq!(
            format_download_filename(None, "resume-builder", std::path::Path::new("r.pdf")),
            "resume_520pdf_resume-builder.pdf"
        );

        let mut opts = HashMap::new();
        opts.insert("target_kb".into(), "100".into());
        assert!(resolve_compress_target_kb(std::path::Path::new("NPWP.pdf"), &opts).is_err());
        opts.insert("target_kb".into(), "250".into());
        assert_eq!(
            resolve_compress_target_kb(std::path::Path::new("NPWP.pdf"), &opts).unwrap(),
            250
        );
        let mut preset_opts = HashMap::new();
        preset_opts.insert("quality_preset".into(), "high".into());
        assert_eq!(
            resolve_compress_target_kb(std::path::Path::new("NPWP.pdf"), &preset_opts).unwrap(),
            250
        );
    }

    #[tokio::test]
    async fn api_token_auth_custom_compress_and_revocation_e2e() {
        let web_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            web_dir.path().join("index.html"),
            "<!doctype html><html>520pdf</html>",
        )
        .unwrap();
        let auth = Arc::new(AuthConfig::disabled_with_dir(data_dir.path().to_path_buf()));
        let router = build_router_with_auth(web_dir.path().to_path_buf(), auth);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();
        let make_multipart = |fields: &[(&str, &str)], file: Option<(&str, &[u8])>| -> Vec<u8> {
            let mut body = Vec::new();
            for (k, v) in fields {
                body.extend_from_slice(
                    format!(
                        "--B520PDF\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n"
                    )
                    .as_bytes(),
                );
            }
            if let Some((fname, bytes)) = file {
                body.extend_from_slice(
                    format!(
                        "--B520PDF\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{fname}\"\r\nContent-Type: application/pdf\r\n\r\n"
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(bytes);
                body.extend_from_slice(b"\r\n");
            }
            body.extend_from_slice(b"--B520PDF--\r\n");
            body
        };

        // /api/tokens requires authenticated session
        let unauth_tokens = client
            .get(format!("{base}/api/tokens"))
            .send()
            .await
            .unwrap();
        assert_eq!(unauth_tokens.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Register Argon2id account -> session cookie returned
        let reg = client
            .post(format!("{base}/api/auth/register"))
            .json(&json!({"username": "tester520", "password": "StrongPassword520!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(reg.status(), reqwest::StatusCode::CREATED);
        let session_cookie = reg
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();

        // Mint a scoped token for compress only
        let created: Value = client
            .post(format!("{base}/api/tokens"))
            .header(header::COOKIE, &session_cookie)
            .json(&json!({"name": "Compress Only", "scopes": ["compress"], "expires_in_days": 30}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let token = created["token"].as_str().unwrap().to_owned();
        let token_id = created["meta"]["id"].as_str().unwrap().to_owned();
        assert!(token.starts_with("520pdf_k_"));
        assert_eq!(created["meta"]["revealable"].as_bool(), Some(true));

        // Reveal requires session + matching login password
        let unauth_reveal = client
            .post(format!("{base}/api/tokens/{token_id}/reveal"))
            .json(&json!({"password": "StrongPassword520!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(unauth_reveal.status(), reqwest::StatusCode::UNAUTHORIZED);

        let wrong_pass_reveal = client
            .post(format!("{base}/api/tokens/{token_id}/reveal"))
            .header(header::COOKIE, &session_cookie)
            .json(&json!({"password": "WrongPassword123!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            wrong_pass_reveal.status(),
            reqwest::StatusCode::UNAUTHORIZED
        );

        let good_reveal = client
            .post(format!("{base}/api/tokens/{token_id}/reveal"))
            .header(header::COOKIE, &session_cookie)
            .json(&json!({"password": "StrongPassword520!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(good_reveal.status(), reqwest::StatusCode::OK);
        assert_eq!(
            good_reveal
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store")
        );
        let good_reveal_json: Value = good_reveal.json().await.unwrap();
        assert_eq!(good_reveal_json["token"].as_str(), Some(token.as_str()));

        // Out-of-scope tool returns 403 Forbidden
        let resume_pdf = client
            .post(format!("{base}/api/v1/tools/resume-builder"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=B520PDF",
            )
            .body(make_multipart(
                &[(
                    "resume_json",
                    r#"{"name":"API Test User","summary":"Compress DPI check"}"#,
                )],
                None,
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resume_pdf.status(), reqwest::StatusCode::FORBIDDEN);

        // Invalid token always rejected 401
        let bad_tok = client
            .post(format!("{base}/api/v1/tools/compress"))
            .header(header::AUTHORIZATION, "Bearer 520pdf_k_00000000_invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(bad_tok.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Create sample PDF, then compress via Bearer token + custom DPI & quality
        let pdf_bytes = client
            .post(format!("{base}/api/tools/resume-builder"))
            .header(
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=B520PDF",
            )
            .body(make_multipart(
                &[(
                    "resume_json",
                    r#"{"name":"API Test User","summary":"Custom DPI & quality"}"#,
                )],
                None,
            ))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();

        let compress_res = client
            .post(format!("{base}/api/v1/tools/compress"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=B520PDF",
            )
            .body(make_multipart(
                &[("target_kb", "300"), ("dpi", "96"), ("quality", "55")],
                Some(("sample.pdf", &pdf_bytes)),
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(compress_res.status(), reqwest::StatusCode::OK);
        assert_eq!(
            compress_res
                .headers()
                .get("x-520pdf-token-id")
                .unwrap()
                .to_str()
                .unwrap(),
            token_id
        );
        let compressed = compress_res.bytes().await.unwrap();
        assert!(compressed.starts_with(b"%PDF-"));

        // Revoke token -> subsequent Bearer/X-API-Key call returns 401
        let revoked = client
            .delete(format!("{base}/api/tokens/{token_id}"))
            .header(header::COOKIE, &session_cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(revoked.status(), reqwest::StatusCode::OK);
        let reveal_after_revoke = client
            .post(format!("{base}/api/tokens/{token_id}/reveal"))
            .header(header::COOKIE, &session_cookie)
            .json(&json!({"password": "StrongPassword520!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            reveal_after_revoke.status(),
            reqwest::StatusCode::BAD_REQUEST
        );
        let after_revoke = client
            .post(format!("{base}/api/v1/tools/compress"))
            .header("x-api-key", &token)
            .send()
            .await
            .unwrap();
        assert_eq!(after_revoke.status(), reqwest::StatusCode::UNAUTHORIZED);

        // OpenAPI + Docs search endpoints respond cleanly
        let oa: Value = client
            .get(format!("{base}/api/openapi.json"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(oa["paths"]["/api/v1/tools/compress"].is_object());
        let search: Value = client
            .get(format!("{base}/api/docs/search?query=compress"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(!search.as_array().unwrap().is_empty());

        handle.abort();
    }

    #[tokio::test]
    async fn api_cors_and_job_progress_e2e() {
        let web_dir = tempfile::tempdir().unwrap();
        let data_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            web_dir.path().join("index.html"),
            "<!doctype html><html>520pdf</html>",
        )
        .unwrap();
        let auth = Arc::new(AuthConfig::disabled_with_dir(data_dir.path().to_path_buf()));
        let router = build_router_with_auth(web_dir.path().to_path_buf(), auth);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();

        // 1. Default CORS is '*' on preflight and GET /api/*
        let preflight = client
            .request(
                reqwest::Method::OPTIONS,
                format!("{base}/api/v1/tools/compress"),
            )
            .header(header::ORIGIN, "https://any-client.example")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .send()
            .await
            .unwrap();
        assert_eq!(preflight.status(), reqwest::StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("*")
        );
        assert!(
            preflight
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|m| m.contains("POST"))
        );

        let status_res = client
            .get(format!("{base}/api/status"))
            .header(header::ORIGIN, "https://any-client.example")
            .send()
            .await
            .unwrap();
        assert_eq!(
            status_res
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("*")
        );

        // 2. Authenticated PUT /api/cors updates explicit allowlist
        let reg = client
            .post(format!("{base}/api/auth/register"))
            .json(&json!({"username": "corsadmin", "password": "StrongPassword520!"}))
            .send()
            .await
            .unwrap();
        assert_eq!(reg.status(), reqwest::StatusCode::CREATED);
        let cookie = reg
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();

        let upd = client
            .put(format!("{base}/api/cors"))
            .header(header::COOKIE, &cookie)
            .json(&json!({"origins": ["https://trusted.example"]}))
            .send()
            .await
            .unwrap();
        assert_eq!(upd.status(), reqwest::StatusCode::OK);

        let allowed = client
            .request(
                reqwest::Method::OPTIONS,
                format!("{base}/api/v1/tools/compress"),
            )
            .header(header::ORIGIN, "https://trusted.example")
            .send()
            .await
            .unwrap();
        assert_eq!(
            allowed
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("https://trusted.example")
        );
        assert_eq!(
            allowed
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .and_then(|v| v.to_str().ok()),
            Some("true")
        );

        let blocked = client
            .request(
                reqwest::Method::OPTIONS,
                format!("{base}/api/v1/tools/compress"),
            )
            .header(header::ORIGIN, "https://blocked.example")
            .send()
            .await
            .unwrap();
        assert!(
            blocked
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );

        // 3. Job progress polling endpoint responds cleanly
        let prog: Value = client
            .get(format!("{base}/api/jobs/probe_123"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(prog["job_id"], "probe_123");
        assert!(prog["percent"].is_u64());
        assert!(prog["stage"].is_string());

        handle.abort();
    }
}
