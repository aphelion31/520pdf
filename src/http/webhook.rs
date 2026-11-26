//! HTTP webhook: `compress-pdf serve`
//!
//! POST /compress   multipart: `file` (pdf, required), `target_kb` (optional, default TARGET_KB)
//!                  header `Authorization: Bearer $WEBHOOK_SECRET`
//!                  200 -> application/pdf body, headers X-Compress-{Status,Dpi,Quality,Size}
//! GET  /health     200 "ok"
//!
//! env: PORT=8080  WEBHOOK_SECRET (required)  TARGET_KB=250  MAX_UPLOAD_MB=50
//!      MAX_JOBS=2 (parallel compressions)  JOB_TIMEOUT_SECS=120

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine;
use std::{process::ExitCode, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

struct DocsAuth {
    user: String,
    password_hash: String,
}

struct App {
    secret: String,
    docs_auth: Option<DocsAuth>,
    target_kb: f64,
    jobs: Arc<Semaphore>,
    timeout: Duration,
}

fn env<T: std::str::FromStr>(k: &str, default: T) -> T {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn main() -> ExitCode {
    let secret = std::env::var("WEBHOOK_SECRET").unwrap_or_default();
    if secret.len() < 16 {
        eprintln!("error: WEBHOOK_SECRET must be set (>= 16 chars)");
        return ExitCode::FAILURE;
    }
    let docs_auth = std::env::var("DOCS_USER")
        .ok()
        .filter(|u| !u.is_empty() && !u.contains(':'))
        .and_then(|user| {
            if let Ok(hash) = std::env::var("DOCS_PASSWORD_HASH")
                && !hash.trim().is_empty()
            {
                return Some(DocsAuth {
                    user,
                    password_hash: hash,
                });
            }
            std::env::var("DOCS_PASSWORD")
                .ok()
                .filter(|p| !p.is_empty())
                .and_then(|p| crate::http::auth::hash_password(&p).ok())
                .map(|password_hash| DocsAuth {
                    user,
                    password_hash,
                })
        });
    let app = Arc::new(App {
        docs_auth,
        secret,
        target_kb: env("TARGET_KB", 250.0),
        jobs: Arc::new(Semaphore::new(env("MAX_JOBS", 2usize).max(1))),
        timeout: Duration::from_secs(env("JOB_TIMEOUT_SECS", 120)),
    });
    let max_upload = env("MAX_UPLOAD_MB", 50usize) << 20;
    let port: u16 = env("PORT", 8080);

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async move {
        let router = Router::new()
            .route("/", get(|| async { "compress-pdf webhook\n\nPOST /compress  multipart field `file` (PDF), optional `target_kb`\nHeader: Authorization: Bearer <secret>\nGET  /health\n" }))
            .route("/docs", get(docs))
            .route("/docs/", get(docs))
            .route("/docs/openapi.json", get(openapi))
            .route("/health", get(|| async { "ok" }))
            .route("/compress", post(compress))
            .layer(DefaultBodyLimit::max(max_upload))
            .with_state(app);
        let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("error: bind :{port}: {e}");
                return ExitCode::FAILURE;
            }
        };
        println!("compress-pdf webhook listening on :{port}");
        match axum::serve(listener, router)
            .with_graceful_shutdown(shutdown())
            .await
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    })
}

fn docs_response(
    app: &App,
    headers: &HeaderMap,
    content_type: &'static str,
    body: &'static str,
) -> Response {
    let authorized = app.docs_auth.as_ref().is_some_and(|expected| {
        let Some(raw) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Basic "))
        else {
            return false;
        };
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(raw) else {
            return false;
        };
        let Ok(creds) = std::str::from_utf8(&decoded) else {
            return false;
        };
        let Some((user, pass)) = creds.split_once(':') else {
            return false;
        };
        crate::http::auth::constant_time_eq(user.as_bytes(), expected.user.as_bytes())
            && crate::http::auth::verify_password(pass, &expected.password_hash)
    });
    let mut res = if authorized {
        ([(header::CONTENT_TYPE, content_type)], body).into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                "Basic realm=\"PDF documentation\", charset=\"UTF-8\"",
            )],
            "Login required",
        )
            .into_response()
    };
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    res
}

async fn docs(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    docs_response(
        &app,
        &headers,
        "text/html; charset=utf-8",
        include_str!("docs.html"),
    )
}

async fn openapi(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    docs_response(
        &app,
        &headers,
        "application/json",
        include_str!("openapi.json"),
    )
}

async fn shutdown() {
    let term = async {
        #[cfg(unix)]
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
            return;
        }
        std::future::pending::<()>().await
    };
    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term => {} }
}

fn err(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, msg.into()).into_response()
}

/// Constant-time compare so the secret can't be guessed byte by byte from timing.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Keep only safe filename chars for Content-Disposition.
fn clean_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let s: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "._- ".contains(*c))
        .collect();
    let s = s.trim().trim_end_matches(".pdf").to_string();
    if s.is_empty() { "document".into() } else { s }
}

async fn compress(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !same(token.as_bytes(), app.secret.as_bytes()) {
        return err(StatusCode::UNAUTHORIZED, "unauthorized");
    }

    let (mut pdf, mut name, mut target_kb): (Option<Bytes>, String, f64) =
        (None, String::new(), app.target_kb);
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return err(e.status(), e.body_text()),
        };
        match field.name() {
            Some("file") => {
                name = clean_name(field.file_name().unwrap_or(""));
                match field.bytes().await {
                    Ok(b) => pdf = Some(b),
                    Err(e) => return err(e.status(), e.body_text()),
                }
            }
            Some("target_kb") => {
                let v = field.text().await.unwrap_or_default();
                match v.trim().parse::<f64>() {
                    Ok(t) if t > 0.0 && t <= 100_000.0 => target_kb = t,
                    _ => {
                        return err(
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "target_kb must be a number in (0, 100000]",
                        );
                    }
                }
            }
            _ => {}
        }
    }
    let Some(pdf) = pdf else {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing multipart field `file`",
        );
    };
    if !pdf.starts_with(b"%PDF-") {
        return err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "file is not a PDF");
    }

    // CPU-heavy: bound concurrency, run off the async threads
    let Ok(permit) = app.jobs.clone().acquire_owned().await else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "shutting down");
    };
    let limit = (target_kb * 1000.0) as usize;
    let in_size = pdf.len();
    let job = tokio::task::spawn_blocking(move || {
        let _permit = permit; // held until the job really ends, even after a timeout
        let dir = tempdir()?;
        let input = dir.join("input.pdf");
        std::fs::write(&input, &pdf).map_err(|e| e.to_string())?;
        let r = crate::compress(&input, limit);
        let _ = std::fs::remove_dir_all(&dir);
        r
    });
    let (dpi, q, out) = match tokio::time::timeout(app.timeout, job).await {
        Err(_) => return err(StatusCode::GATEWAY_TIMEOUT, "compression timed out"),
        Ok(Err(_)) => return err(StatusCode::INTERNAL_SERVER_ERROR, "compression crashed"),
        Ok(Ok(Err(e))) => return err(StatusCode::UNPROCESSABLE_ENTITY, e),
        Ok(Ok(Ok(r))) => r,
    };

    let status = if out.len() <= limit {
        "OK"
    } else {
        "OVER_TARGET"
    };
    println!(
        "compress {name}.pdf {} KB -> {} KB [{dpi} dpi, q{q}] {status}",
        in_size / 1000,
        out.len() / 1000
    );
    let size = out.len();
    let mut res = (StatusCode::OK, out).into_response();
    let h = res.headers_mut();
    let hv = |s: String| HeaderValue::from_str(&s).unwrap_or(HeaderValue::from_static(""));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        hv(format!("attachment; filename=\"compressed_{name}.pdf\"")),
    );
    h.insert("x-compress-status", HeaderValue::from_static(status));
    h.insert("x-compress-dpi", hv(dpi.to_string()));
    h.insert("x-compress-quality", hv(q.to_string()));
    h.insert("x-compress-size", hv(size.to_string()));
    res
}

fn tempdir() -> Result<std::path::PathBuf, String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("compress-pdf-upload-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_require_separate_credentials() {
        let mut app = App {
            secret: "api-token".into(),
            docs_auth: Some(DocsAuth {
                user: "user".into(),
                password_hash: crate::http::auth::hash_password("pass").unwrap(),
            }),
            target_kb: 250.0,
            jobs: Arc::new(Semaphore::new(1)),
            timeout: Duration::from_secs(120),
        };
        let mut headers = HeaderMap::new();
        assert_eq!(
            docs_response(&app, &headers, "text/html", "private").status(),
            StatusCode::UNAUTHORIZED
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer api-token"),
        );
        assert_eq!(
            docs_response(&app, &headers, "text/html", "private").status(),
            StatusCode::UNAUTHORIZED
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjp3cm9uZw=="),
        );
        assert_eq!(
            docs_response(&app, &headers, "text/html", "private").status(),
            StatusCode::UNAUTHORIZED
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        let response = docs_response(&app, &headers, "text/html", "private");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        app.docs_auth = None;
        assert_eq!(
            docs_response(&app, &headers, "text/html", "private").status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn secret_compare_and_names() {
        assert!(same(b"abc", b"abc") && !same(b"abc", b"abd") && !same(b"abc", b"ab"));
        assert_eq!(clean_name("../../etc/pass\"wd.pdf"), "passwd");
        assert_eq!(clean_name("C:\\x\\Laporan Akhir.pdf"), "Laporan Akhir");
        assert_eq!(clean_name(""), "document");
    }
}
