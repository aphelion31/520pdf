use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

struct App {
    jobs: Arc<Semaphore>,
}
fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}

pub fn main() -> std::process::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let state = Arc::new(App {
            jobs: Arc::new(Semaphore::new(2)),
        });
        let app = Router::new()
            .route(
                "/",
                get(|| async { axum::response::Html(include_str!("../static/index.html")) }),
            )
            .route(
                "/assets/app.css",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "text/css")],
                        include_str!("../static/app.css"),
                    )
                }),
            )
            .route(
                "/assets/basecoat.css",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "text/css")],
                        include_str!("../static/basecoat.css"),
                    )
                }),
            )
            .route(
                "/assets/app.js",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/javascript")],
                        include_str!("../static/app.js"),
                    )
                }),
            )
            .route(
                "/assets/pdfjs/pdf.mjs",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/javascript")],
                        include_str!("../static/pdfjs/pdf.mjs"),
                    )
                }),
            )
            .route(
                "/assets/pdfjs/pdf.worker.mjs",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/javascript")],
                        include_str!("../static/pdfjs/pdf.worker.mjs"),
                    )
                }),
            )
            .route(
                "/assets/workbench.js",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/javascript")],
                        include_str!("../static/workbench.js"),
                    )
                }),
            )
            .route(
                "/assets/workbench.css",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "text/css")],
                        include_str!("../static/workbench.css"),
                    )
                }),
            )
            .route("/health", get(|| async { "ok" }))
            .route("/api/tools", get(|| async { Json(catalog()) }))
            .route("/api/tools/{tool}", post(process))
            .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
            .with_state(state);
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);
        let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| format!("127.0.0.1:{port}"));
        let listener = match tokio::net::TcpListener::bind(&bind).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("bind {bind}: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        println!("520pdf listening on {bind}");
        match axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
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

async fn process(
    State(app): State<Arc<App>>,
    Path(tool): Path<String>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
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
    let mut options = HashMap::new();
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return error(e.status(), "Invalid or oversized upload"),
        };
        let name = field.name().unwrap_or("").to_owned();
        if field.file_name().is_some() {
            if inputs.len() >= 30 {
                return error(StatusCode::BAD_REQUEST, "Maximum 30 files per job");
            }
            let ext = std::path::Path::new(field.file_name().unwrap())
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
                Ok(v) if v.len() <= 100_000 => {
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
    if inputs.is_empty() {
        return error(StatusCode::UNPROCESSABLE_ENTITY, "Select at least one file");
    }
    let job = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut compression = None;
        let result = if tool == "compress" {
            let kb = options
                .get("target_kb")
                .map(|v| v.parse::<usize>())
                .transpose()
                .map_err(|_| "Invalid target KB".to_string())?
                .unwrap_or(250);
            if !(10..=100000).contains(&kb) {
                return Err("Target must be 10–100000 KB".into());
            }
            if inputs.len() != 1 {
                return Err("Compression requires exactly one PDF".into());
            }
            let (dpi, quality, bytes) = crate::compress(&inputs[0], kb * 1000)?;
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
        } else if matches!(
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
        ) {
            crate::tools::run(&tool, &inputs, &options, dir.path())
        } else if matches!(tool.as_str(), "summarize" | "translate") {
            crate::ai_tools::run(&tool, &inputs, &options, dir.path())
        } else {
            crate::conversions::run(&tool, &inputs, &options, dir.path())
        };
        let (path, mime) = result?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("520pdf-output.pdf")
            .replace(['"', '\r', '\n'], "");
        Ok::<_, String>((bytes, mime, filename, compression))
    });
    match tokio::time::timeout(std::time::Duration::from_secs(180), job).await {
        Ok(Ok(Ok((bytes, mime, name, compression)))) => {
            let mut res = Response::new(Body::from(bytes));
            let h = res.headers_mut();
            if let Some((status, dpi, quality)) = compression {
                h.insert("x-compress-status", status.parse().unwrap());
                h.insert("x-compress-dpi", dpi.to_string().parse().unwrap());
                h.insert("x-compress-quality", quality.to_string().parse().unwrap());
            }
            if let Ok(v) = mime.parse() {
                h.insert(header::CONTENT_TYPE, v);
            }
            if let Ok(v) = format!("attachment; filename=\"{name}\"").parse() {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            h.insert("x-content-type-options", "nosniff".parse().unwrap());
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
fn catalog() -> Value {
    let rows = [
        (
            "merge",
            "Merge PDF",
            "Combine PDFs in upload order.",
            "Organize",
            ".pdf",
            true,
        ),
        (
            "split",
            "Split PDF",
            "Extract selected pages.",
            "Organize",
            ".pdf",
            false,
        ),
        (
            "organize",
            "Organize PDF",
            "Reorder, duplicate or remove pages.",
            "Organize",
            ".pdf",
            false,
        ),
        (
            "compress",
            "Compress PDF",
            "Reduce PDF file size.",
            "Optimize",
            ".pdf",
            false,
        ),
        (
            "image-to-pdf",
            "Images to PDF",
            "Turn photos into a PDF.",
            "Convert",
            ".jpg,.jpeg,.png,.webp",
            true,
        ),
        (
            "office-to-pdf",
            "Office to PDF",
            "Convert Word, PowerPoint and spreadsheets.",
            "Convert",
            ".doc,.docx,.ppt,.pptx,.xls,.xlsx,.odt",
            false,
        ),
        (
            "crop",
            "Crop PDF",
            "Change visible boundaries only, not secure redaction.",
            "Edit",
            ".pdf",
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
            "Extract document text as Markdown.",
            "Convert",
            ".pdf",
            false,
        ),
        (
            "sign",
            "Sign PDF",
            "Add a visible signature, not a digital certificate.",
            "Edit",
            ".pdf",
            false,
        ),
        (
            "watermark",
            "Watermark PDF",
            "Stamp text onto your pages.",
            "Edit",
            ".pdf",
            false,
        ),
        (
            "rotate",
            "Rotate PDF",
            "Rotate document pages.",
            "Organize",
            ".pdf",
            false,
        ),
        (
            "html-to-pdf",
            "HTML to PDF",
            "Render uploaded HTML and CSS without external resource access.",
            "Convert",
            ".html,.htm",
            false,
        ),
        (
            "unlock",
            "Unlock PDF",
            "Remove encryption with the known password.",
            "Security",
            ".pdf",
            false,
        ),
        (
            "protect",
            "Protect PDF",
            "Encrypt PDF with a password.",
            "Security",
            ".pdf",
            false,
        ),
        (
            "pdfa",
            "PDF to PDF/A",
            "Convert to PDF/A-2b and require veraPDF compliance validation.",
            "Convert",
            ".pdf",
            false,
        ),
        (
            "scan-to-pdf",
            "Scan to PDF",
            "OCR scanned pages into searchable PDF.",
            "Convert",
            ".pdf,.jpg,.jpeg,.png",
            false,
        ),
        (
            "page-numbers",
            "Page numbers",
            "Number the pages of a PDF.",
            "Edit",
            ".pdf",
            false,
        ),
        (
            "repair",
            "Repair PDF",
            "Recover a readable PDF structure.",
            "Optimize",
            ".pdf",
            false,
        ),
        (
            "forms",
            "PDF forms",
            "Fill existing named text fields.",
            "Edit",
            ".pdf",
            false,
        ),
        (
            "redact",
            "Redact PDF",
            "Rasterize pages and permanently remove selected rectangles.",
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
    let tools:Vec<Value>=rows.into_iter().map(|(id,name,desc,cat,accept,multiple)|{
        let fields=match id {
            "compress"=>vec![field("target_kb","Target size (KB)","number","250")],
            "split"=>vec![field("pages","Pages (example: 1,3-5,2)","text","1"),field("mode","Split mode (extract, all, ranges)","text","extract"),field("ranges","Separate output groups (example: 1-2;3-4)","text","")],
            "organize"=>vec![field("pages","Page order, duplicates allowed (1,3,1)","text","1")],
            "rotate"=>vec![field("angle","Clockwise rotation (90, 180, 270)","number","90")],
            "crop"=>vec![field("x","Left coordinate (points)","number","20"),field("y","Bottom coordinate (points)","number","20"),field("width","Crop width (points)","number","550"),field("height","Crop height (points)","number","750")],
            "image-to-pdf"=>vec![field("page_size","Page size (fit, a4, letter)","text","a4"),field("orientation","Orientation (portrait, landscape)","text","portrait"),field("margin","Margin (PDF points)","number","24")],
            "watermark"|"sign"=>vec![field("text",if id=="sign"{"Signature text"}else{"Watermark text"},"text",""),field("x","X position (points)","number","36"),field("y","Y position from bottom (points)","number","72"),field("width","Signature width (points)","number","160"),field("height","Signature height (points)","number","60"),field("font_size","Text size (points)","number","32"),field("rotation","Text rotation (degrees)","number","0"),field("opacity","Opacity (0 to 1)","number","0.5")],
            "page-numbers"=>vec![field("start","Start number","number","1"),field("format","Format ({n}, {total})","text","Page {n} of {total}"),field("position","Position (left, center, right)","text","center"),field("y","Y from bottom (points)","number","24")],
            "protect"=>vec![field("password","User password","password",""),field("owner_password","Owner password (must differ)","password",""),field("print","Printing (full, low, none)","text","full"),field("modify","Changes (all, annotate, form, assembly, none)","text","all"),field("extract","Allow extraction (y, n)","text","y")],
            "unlock"=>vec![field("password","PDF password","password","")],
            "forms"=>vec![field("action","Action (fill, discover)","text","fill"),field("fields","Field values (JSON object)","textarea","{}")],
            "redact"=>vec![field("rectangles","Redaction rectangles (JSON; page, x, y, width, height)","textarea","[]")],
            "scan-to-pdf"=>vec![field("language","OCR language","text","eng"),field("deskew","Straighten scanned pages","checkbox","true")],
            "summarize"|"translate"=>vec![field("target_language","Translation language","text","English"),field("summary_language","Summary language","text","English"),field("summary_length","Summary length (short, medium, long)","text","medium"),field("output_format","Output format (txt, pdf)","text","pdf"),field("bilingual","Include original text with translation","checkbox","false"),field("ocr","OCR scanned text before AI","checkbox","false"),field("ocr_language","OCR language","text","eng"),field("consent","I consent to send extracted document text to the configured AI provider","checkbox","false")],
            _=>vec![]
        };
        json!({"id":id,"name":name,"description":desc,"category":cat,"accept":accept,"multiple":multiple,"fields":fields})
    }).collect();
    json!({"tools":tools})
}
