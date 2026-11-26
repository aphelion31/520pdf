use crate::http::auth::{self, AuthConfig};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{SecondsFormat, Utc};
use lopdf::Document;
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};

struct App {
    jobs: Arc<Semaphore>,
    auth: Arc<AuthConfig>,
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}

pub fn build_router(web_dir: PathBuf) -> Router {
    let auth = Arc::new(AuthConfig::from_env().expect("invalid AUTH_* configuration"));
    build_router_with_auth(web_dir, auth)
}

pub fn build_router_with_auth(web_dir: PathBuf, auth: Arc<AuthConfig>) -> Router {
    let state = Arc::new(App {
        jobs: Arc::new(Semaphore::new(2)),
        auth: auth.clone(),
    });
    let index_html = web_dir.join("index.html");
    let spa = ServeDir::new(&web_dir).fallback(ServeFile::new(index_html));

    let auth_router = Router::new()
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .with_state(auth);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ready", get(|| async { "ok" }))
        .route("/api/status", get(api_status))
        .route("/api/tools", get(|| async { Json(catalog()) }))
        .route("/api/tools/{tool}", post(process))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(state)
        .merge(auth_router)
        .fallback_service(spa)
        .layer(CompressionLayer::new())
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
    mut form: Multipart,
) -> Response {
    if !app.auth.is_authorized(&headers) {
        return error(StatusCode::UNAUTHORIZED, "Authentication required");
    }
    // Browser requests must be same-origin. Non-browser API clients omit Origin.
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if origin != format!("https://{host}") && origin != format!("http://{host}") {
            return error(StatusCode::FORBIDDEN, "Cross-origin upload rejected");
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
    let permit = match app.jobs.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
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
            match field.text().await {
                Ok(v) if v.len() <= 200_000 => {
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
        return error(StatusCode::UNPROCESSABLE_ENTITY, "Select at least one file");
    }
    let job = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut compression = None;
        let visual_protect = tool == "protect"
            && (options.get("mode").map(String::as_str) == Some("visual")
                || options.get("password").is_none_or(|p| p.trim().is_empty()));
        let result = if tool == "compress" {
            if inputs.len() != 1 {
                return Err("Compression requires exactly one PDF".into());
            }
            let kb = resolve_compress_target_kb(&inputs[0], &options)?;
            let (dpi, quality, mut bytes) = crate::compress(&inputs[0], kb * 1000)?;
            if options.get("remove_metadata").map(String::as_str) == Some("true")
                && let Ok(mut doc) = Document::load_mem(&bytes)
            {
                crate::tools::strip_metadata(&mut doc);
                let mut stripped = Vec::new();
                if doc.save_to(&mut stripped).is_ok() {
                    bytes = stripped;
                }
            }
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
        } else if visual_protect
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
            )
        {
            crate::tools::run(&tool, &inputs, &options, dir.path())
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
        let (path, mime) = result?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let filename = format_download_filename(first_upload_name.as_deref(), &tool, &path);
        Ok::<_, String>((bytes, mime, filename, compression))
    });
    match tokio::time::timeout(std::time::Duration::from_secs(180), job).await {
        Ok(Ok(Ok((bytes, mime, name, compression)))) => {
            let mut res = Response::new(Body::from(bytes));
            let h = res.headers_mut();
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
        Ok(Ok(Err(e))) => error(StatusCode::UNPROCESSABLE_ENTITY, e),
        Ok(Err(_)) => error(StatusCode::INTERNAL_SERVER_ERROR, "Processing failed"),
        Err(_) => error(
            StatusCode::GATEWAY_TIMEOUT,
            "Job exceeded 180 seconds. Worker slot stays reserved until processing stops.",
        ),
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
        "equation-to-image" => "/equation-to-image".into(),
        "latex-resume" => "/latex-resume".into(),
        "resume-builder" => "/resume-builder".into(),
        other => format!("/tools/{other}"),
    }
}

pub fn catalog() -> Value {
    let rows = [
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
            "Export selected PDF pages as PNG, JPG, or WebP images.",
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
            "Add visual protection overlays + metadata strip or AES-256 encryption.",
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
            "Rotate selected document pages 90, 180, or 270 degrees.",
            "Organize",
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
            "Discover and fill named text or checkbox form fields.",
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
            "Compare extracted text and rasterized page appearance.",
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
            "Rebuild damaged xref tables and recover readable structure.",
            "Optimize",
            ".pdf",
            false,
        ),
        (
            "redact",
            "Redact PDF",
            "Rasterize pages and permanently black out selected regions.",
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
    let tools: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, desc, cat, accept, multiple)| {
            let fields = match id {
                "pdf-to-image" => vec![
                    field("pages", "Pages (blank means all)", "text", ""),
                    select_field(
                        "dpi",
                        "Resolution (DPI)",
                        "150",
                        &[
                            ("72", "72 DPI (Screen)"),
                            ("150", "150 DPI (Standard)"),
                            ("200", "200 DPI (High)"),
                            ("300", "300 DPI (Print)"),
                        ],
                    ),
                    select_field(
                        "format",
                        "Image format",
                        "png",
                        &[("png", "PNG"), ("jpeg", "JPEG"), ("webp", "WebP")],
                    ),
                ],
                "compress-image" => vec![
                    field("target_kb", "Target size (KB)", "number", "250"),
                    field("quality", "Quality (1-100)", "number", "80"),
                    field("max_width", "Max width (px)", "number", "1920"),
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
                    field(
                        "expression",
                        "LaTeX math expression",
                        "textarea",
                        "E = mc^2",
                    ),
                    field("font_size", "Font size", "number", "32"),
                ],
                "latex-resume" => vec![field("latex", "LaTeX source", "textarea", "")],
                "resume-builder" => vec![field("resume_json", "Resume JSON", "textarea", "")],
                "compress" => vec![json!({
                    "name": "target_kb",
                    "label": "Target size (KB, min 250)",
                    "type": "number",
                    "value": "250",
                    "min": 250,
                    "max": 100000,
                    "step": 1,
                    "help": "Minimum compression target is 250 KB."
                })],
                "split" => vec![
                    field("pages", "Pages (example: 1,3-5,2)", "text", "1"),
                    select_field(
                        "mode",
                        "Split mode",
                        "extract",
                        &[
                            ("extract", "Extract selected pages"),
                            ("all", "Burst all pages to ZIP"),
                            ("ranges", "Separate range groups"),
                        ],
                    ),
                    field(
                        "ranges",
                        "Separate output groups (example: 1-2;3-4)",
                        "text",
                        "",
                    ),
                ],
                "organize" => vec![field(
                    "pages",
                    "Page order, duplicates allowed (1,3,1)",
                    "text",
                    "1",
                )],
                "rotate" => vec![select_field(
                    "angle",
                    "Clockwise rotation",
                    "90",
                    &[
                        ("90", "90° Clockwise"),
                        ("180", "180° Flip"),
                        ("270", "270° Counter-clockwise"),
                    ],
                )],
                "crop" => vec![
                    field("x", "Left coordinate (points)", "number", "20"),
                    field("y", "Bottom coordinate (points)", "number", "20"),
                    field("width", "Crop width (points)", "number", "550"),
                    field("height", "Crop height (points)", "number", "750"),
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
                    field("margin", "Margin (PDF points)", "number", "24"),
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
                        field("x", "X position (points)", "number", "36"),
                        field("y", "Y position from bottom (points)", "number", "72"),
                        field("width", "Width (points)", "number", "160"),
                        field("height", "Height (points)", "number", "60"),
                        field("font_size", "Text size (points)", "number", "32"),
                        field("rotation", "Text rotation (degrees)", "number", "0"),
                        field("opacity", "Opacity (0 to 1)", "number", "0.5"),
                    ];
                    if id == "watermark" {
                        list.push(field(
                            "color",
                            "Text color (hex #RRGGBB)",
                            "text",
                            "#E63228",
                        ));
                        list.push(field("count", "Stamp count (1-64)", "number", "1"));
                        list.push(field("cols", "Grid columns (1-8)", "number", "1"));
                        list.push(field("rows", "Grid rows (1-10)", "number", "1"));
                    }
                    list
                }
                "page-numbers" => vec![
                    field("start", "Start number", "number", "1"),
                    field(
                        "format",
                        "Format ({n}, {total})",
                        "text",
                        "Page {n} of {total}",
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
                    field("y", "Y from bottom (points)", "number", "24"),
                ],
                "protect" => vec![
                    select_field(
                        "mode",
                        "Protection mode",
                        "visual",
                        &[
                            ("visual", "Visual stamp + strip metadata"),
                            ("password", "AES-256 password encryption"),
                        ],
                    ),
                    field(
                        "watermark_text",
                        "Visual watermark text",
                        "text",
                        "CONFIDENTIAL",
                    ),
                    field(
                        "remove_metadata",
                        "Strip document metadata",
                        "checkbox",
                        "true",
                    ),
                    field("password", "User password (AES-256 mode)", "password", ""),
                    field(
                        "owner_password",
                        "Owner password (must differ)",
                        "password",
                        "",
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
                        "Allow text extraction",
                        "y",
                        &[("y", "Yes"), ("n", "No")],
                    ),
                ],
                "unlock" => vec![field("password", "PDF password", "password", "")],
                "forms" => vec![
                    select_field(
                        "action",
                        "Action",
                        "fill",
                        &[
                            ("fill", "Fill form fields"),
                            ("discover", "Discover field names JSON"),
                        ],
                    ),
                    field("fields", "Field values (JSON object)", "textarea", "{}"),
                ],
                "redact" => vec![field(
                    "rectangles",
                    "Redaction rectangles (JSON; page, x, y, width, height)",
                    "textarea",
                    "[]",
                )],
                "scan-to-pdf" => vec![
                    select_field(
                        "language",
                        "OCR language",
                        "eng",
                        &[
                            ("eng", "English (eng)"),
                            ("ind", "Indonesian (ind)"),
                            ("vie", "Vietnamese (vie)"),
                            ("ind+eng", "Indonesian + English (ind+eng)"),
                        ],
                    ),
                    field("deskew", "Straighten scanned pages", "checkbox", "true"),
                ],
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
                    field("ocr", "OCR scanned text before AI", "checkbox", "false"),
                    select_field(
                        "ocr_language",
                        "OCR language",
                        "eng",
                        &[
                            ("eng", "English (eng)"),
                            ("ind", "Indonesian (ind)"),
                            ("ind+eng", "Indonesian + English"),
                        ],
                    ),
                    field(
                        "consent",
                        "I consent to send extracted document text to the configured AI provider",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_28_tools_and_routes() {
        let cat = catalog();
        let tools = cat["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 28);
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
        let router = build_router(tmp.path().to_path_buf());
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
        assert_eq!(tools_body["tools"].as_array().unwrap().len(), 28);

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
}
