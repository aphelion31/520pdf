# 520pdf

**520pdf: 我爱你 PDF (wǒ ài nǐ PDF), "I love you, PDF."**

In Chinese internet slang, **520** sounds like **我爱你** (*wǒ ài nǐ*, "I love you").
520pdf turns that affection into a practical, self-hosted PDF workbench.
Its English-language interface uses Basecoat, inspired by familiar tool-directory workflows,
with an independent identity. Not affiliated with iLovePDF.

## Stack

Rust + Axum handle the application and PDF workflows. `lopdf` performs native PDF manipulation,
with the existing Rust/mozjpeg compressor copied from `compresspdf-axum-webhook`.
Specialized document engines (Ghostscript, qpdf, LibreOffice, Poppler, Tesseract) are subprocesses,
not rewritten PDF renderers. REA is an inspection tool used during development, not a frontend framework.
Basecoat 1.1.0 is vendored locally. No React or Node runtime is required in production.

## Run

```bash
cargo run --release -- serve
# http://127.0.0.1:8080
```

Install the engines listed in [docs/TOOLS.md](docs/TOOLS.md) for conversions and OCR.
The Docker image contains the runtime engines:

```bash
cp .env.example .env
docker compose up -d --build
```

No existing `pdf.multimedika.id` deployment is changed by this project.

## Tools

Merge, split, organize, compress, images to PDF, Office to PDF (Word / PowerPoint /
spreadsheets), crop, compare, PDF to Markdown, visible signatures, watermark, rotate,
HTML to PDF, unlock, protect, PDF/A, scan/OCR, page numbers, repair, PDF forms,
redact, AI summarize and translate.

See [docs/TOOLS.md](docs/TOOLS.md) for exact capabilities, engine requirements and limitations.
A visible signature is not a cryptographic digital signature. PDF/A output requires independent
validation before archival use. Compare and Markdown extraction are text-oriented.

## API

- `GET /api/tools`: tool catalog, file types and option fields.
- `POST /api/tools/{id}`: multipart files plus tool-specific options. Returns a download.
- `GET /health`: readiness endpoint.
- `pdf520 input.pdf -t 250 -o output.pdf`: inherited compressor CLI.
- `pdf520 webhook`: inherited authenticated compressor webhook, separate from the fullstack UI.

Uploads are limited to 50 MiB per request, 30 files and two simultaneous processing jobs.
Per-job temporary directories are deleted after work completes. A timed-out blocking job keeps
its slot until the underlying processing ends. Files are not intentionally retained.

## Privacy and deployment

The default bind address is loopback. A public instance requires HTTPS, rate limiting,
container memory/CPU/PID limits, no host filesystem mounts, and current conversion engines.
Treat uploaded documents as untrusted. Do not expose this application with an unrestricted
network path to internal services. PDF parsers and Office converters remain security-sensitive.

AI features are opt-in. Configure your provider explicitly, then affirm consent per request.
Extracted text may contain personal data and provider requests may incur charges. No AI keys,
webhook secrets or document passwords belong in source control.

## Design references

- [Basecoat](https://basecoatui.com/) for accessible shadcn-like HTML components.
- [REA](https://github.com/morluto/rea) for bounded public-site inspection.
- [iLovePDF](https://www.ilovepdf.com/) for the familiar browse/select/upload workflow only.

## Validation

```bash
cargo test
```

Native engine tests exercise generated PDF fixtures without external services.
Conversion integration tests require the engines in the runtime image. AI calls are never
made as part of the default test suite.
