# 520pdf conversion tools

All tools operate on uploaded files in a private temporary directory. The conversion API matches `tools::run`: `(tool, &[PathBuf], &HashMap<String,String>, work_dir) -> Result<(PathBuf, MIME), String>`. Multipart option values are strings. Commands are invoked without a shell, with a 180-second per-command timeout and process-group termination. Missing binaries, failed commands and empty outputs return errors, not fake successes.

## Runtime

`docker/axum/Dockerfile` provides Ubuntu 24.04 with LibreOffice Writer/Calc/Impress, Ghostscript, QPDF, Poppler, ImageMagick, Tesseract, OCRmyPDF, img2pdf and MuPDF. English (`eng`), Indonesian (`ind`) and Vietnamese (`vie`) OCR language packs are installed. Add other `tesseract-ocr-*` packages if needed. Runtime runs as UID 1001, with private writable temporary storage and a writable home for LibreOffice.

Conversion processes parse untrusted documents. Keep image packages patched, drop container capabilities, use `no-new-privileges`, set memory/CPU/PID limits, do not mount sensitive host paths, and restrict container egress. Macro execution is disabled in each isolated LibreOffice profile. Office conversion is not a security sandbox and office files can contain external links. For hostile public uploads, run conversion workers in an OS sandbox with network denied and only request-specific files exposed. AI needs separately allowlisted HTTPS egress. Do not claim office parsing is network-isolated by this application alone.

## Contract and limitations

| Tool ID | Input | Options | Result / behavior |
| --- | --- | --- | --- |
| `office-to-pdf` | One `.doc`, `.docx`, `.ppt`, `.pptx`, `.xls`, `.xlsx`, `.odt` | none | LibreOffice PDF export, no macros. Layout depends on installed fonts. |
| `html-to-pdf` | One UTF-8 HTML file | none | Visible text only. Discards markup, scripts, styles, frames, images and external resources, then forces LibreOffice plain-text import. No HTML/browser engine receives user markup. No CSS/layout fidelity or URL fetching. |
| `compare` | Exactly two PDFs | none | Line-position comparison of Poppler-extracted text. Text report, not image/layout comparison. Scanned pages require OCR first. |
| `pdf-to-markdown` | One PDF | none | UTF-8 text/paragraphs with page separators. No invented headings/table structure. No OCR, errors on empty text. |
| `unlock` | One PDF | `password`, optional for files without an open password | QPDF decrypt. Password must be known. Does not crack encryption. |
| `protect` | One PDF | Required `password`, optional `owner_password` | QPDF AES-256 encryption. Passwords passed in a private temporary job file, not process arguments. |
| `pdfa` | One PDF | none | OCRmyPDF/Ghostscript PDF/A-2 conversion. Fails if conversion cannot be completed. Not a substitute for external archival conformance certification. |
| `scan-to-pdf` | One PDF or supported raster image | `language`, default `eng`, e.g. `ind+eng` | OCRmyPDF searchable PDF, skips existing text. Missing language models produce an error. |
| `repair` | One damaged PDF | none | MuPDF rebuild, then QPDF structural check. Cannot recover missing/truncated content. |
| `redact` | One PDF, 1..100 pages | Required JSON `rectangles` | Rasterizes every page, paints regions black, strips metadata, builds a new image-only PDF. Original PDF objects, text, forms, annotations, attachments are not copied. Searchability/vector fidelity is lost. |
| `summarize` | One text-bearing PDF | Required `consent=true` | Sends extracted text to operator-configured AI, returns text. |
| `translate` | One text-bearing PDF | Required `consent=true`, `target_language` | Sends extracted text to operator-configured AI, returns translated text, not a layout-preserving PDF. |

### Redaction coordinates

Multipart `rectangles` is a JSON array, e.g. `[ { "page": 1, "x": 20, "y": 30, "width": 160, "height": 24 } ]`. Pages are 1-based, coordinates are PDF points measured from the **top-left**. Maximum 1000 rectangles. Rotated, offset or cropped page boxes are rejected because coordinate ambiguity could leave content exposed. Normalize those pages first. Regions outside the page are rejected. Rendered pages are bounded to 4000 pixels on their longest side. Inspect the resulting PDF before distributing it. Redaction is destructive in the downloaded result only, never in the uploaded source.

### AI configuration and privacy

AI defaults to unavailable. Operator environment:

- `AI_API_URL`: full HTTPS OpenAI-compatible chat/completions URL, e.g. `https://api.openai.com/v1/chat/completions`.
- `AI_API_KEY`: operator secret, never supplied through client options.
- `AI_MODEL`: configured model name.

Only exact multipart `consent=true` enables an AI call. All other values are false. Each AI request discloses extracted document text to the configured provider and may incur provider charges. No document bytes/images are uploaded. Text limit is 120000 UTF-8 bytes, over-limit documents fail rather than silently truncate. Empty extraction fails. Redirects are disabled, URL credentials rejected, HTTPS required, request timeout 120 seconds, response limit 2 MB. No provider response bodies or secret keys are surfaced in HTTP error messages. AI can make errors, particularly translation or clinical summaries. User must check material results. Document content is untrusted data, not instructions.

## Checks

`sh tests/conversions-smoke.sh http://127.0.0.1:8080` runs generated-fixture HTTP acceptance checks inside the runtime, including real encryption/decryption, OCR, repair, PDF/A metadata, redaction text absence/pixel blackness and AI consent rejection without calling a provider. The Docker image includes it at `/usr/local/share/pdf520/conversions-smoke.sh`. CLI diagnostic reads are capped at 64 KiB. Docker binds `0.0.0.0:8080` by default, restrict host port publishing as needed.

`cargo test` includes HTML resource-discarding, text-comparison and strict AI consent checks. Full CLI acceptance requires the Ubuntu runtime. An absent local Docker daemon does not prove runtime behavior. Recommended acceptance fixtures: office documents for every supported extension, HTML with localhost/metadata/file resources, two PDFs with changed text, image-only PDF for OCR, encrypted PDF wrong/right passwords, malformed xref repair and redaction with a known secret that must not appear in `pdftotext` or extracted PDF objects.
