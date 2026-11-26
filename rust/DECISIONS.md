# Decisions (iHatePDF -> 520pdf)

## ADR-001: Rename workspace to `520pdf` in place while keeping `pdf520` Cargo package name
Status: accepted (2026-11-26)
Context: The existing Axum workbench lived in `520pdf-axum-fullstack-app/` with Cargo crate name `pdf520` (Cargo prohibits crate/package names starting with a digit). The upstream reference `cu-sanjay/iHatePDF` is MIT-licensed TypeScript + React + Express.
Decision: Rename the workspace directory in-place to `520pdf/` and keep `../iHatePDF` as a clean read-only clone (`LICENSE-IHATEPDF` preserved). Keep Cargo package/binary name `pdf520` while branding all UI, documentation, Docker configuration, and HTTP service identity as `520pdf`.
Consequences: Existing git history stays intact, `cargo` builds cleanly, and upstream `iHatePDF` stays untouched for ground-truth verification and benchmarking.

## ADR-002: Unified Axum 0.8 + React Router 8 SPA architecture
Status: accepted (2026-11-26)
Context: `iHatePDF` uses an Express server (`server/index.ts`, `GET /api/status`) serving a Vite + React SPA (`wouter`, shadcn/Radix, `pdf-lib`, `pdfjs-dist`, `jspdf`, `html2canvas`, `katex`, `html-to-image`). `520pdf` already implements 27 PDF/media/document endpoints on Axum 0.8 (`src/`).
Decision: Replace the legacy vanilla HTML/JS `static/` assets with a modern **React Router 8 + React 19 + Tailwind CSS v4 + Vite 8** SPA in `web/` built with **Bun 1.4.4** into `web/build/client`, served by the single `pdf520` Axum binary via `tower-http` `ServeDir` + SPA fallback (`WEB_DIR`, default `web/build/client`) with gzip/br compression.
Consequences: Matches the proven `pulsar` and `saturn` single-binary Axum + React Router 8 SPA architecture, supports direct URL routes for all 12 `iHatePDF` tools plus all existing `520pdf` workbench tools, and preserves zero Node/Express runtime dependency in production.

## ADR-003: Pure-Rust local engines for 12/12 `iHatePDF` core tools
Status: accepted (2026-11-26)
Context: Previously `resume-builder`, `equation-to-image`, and `pdf-to-image` depended on external `python3`/`weasyprint`, `python3`/`matplotlib`, or `pdftoppm` binaries not present in minimal macOS/local environments. Meanwhile `iHatePDF` also offers `latex-resume` (3 templates), visual `protect` (overlay + metadata strip), image `compress-image` (quality / max-width / format), and interactive `equation-to-image`.
Decision:
- Implement `resume-builder` and `latex-resume` PDF generation in pure Rust using `pdf-writer 0.15` (`src/features/resume.rs`) — zero Python/WeasyPrint/TeX execution.
- Upgrade `compress-image` (`src/features/new_media.rs`) using pure-Rust `image 0.25` + `mozjpeg 0.10` with `quality`, `max_width`, `output_format` (`jpeg`/`webp`/`png`), and `target_kb` support.
- Upgrade `pdf-to-image` (`src/features/new_media.rs`) with `pages` selection, `webp` output, and automatic Ghostscript (`gs`) fallback when `pdftoppm` is absent.
- Support both pure-Rust visual protection + metadata stripping (`mode=visual` via `lopdf`) and `qpdf` AES-256 encryption in `protect`, plus `quality_preset` (`low`/`medium`/`high`) in `compress`.
- Keep interactive KaTeX math preview + pixel-tight PNG/JPG export (`katex` + `html-to-image`) in `web/` alongside the `POST /api/tools/equation-to-image` backend endpoint.
- Retain Docker conversion extras (`office-to-pdf`, `html-to-pdf`, `pdfa`, `scan-to-pdf`, `repair`, `redact`, `pdf-to-markdown`, `compare`, `summarize`, `translate`) as bounded subprocesses.
Consequences: Every core `iHatePDF` tool runs out-of-the-box locally on macOS and Linux with zero Python runtime required.

## ADR-004: HTTP Equivalence Bar & Ground-Truth Verification
Status: accepted (2026-11-26)
Context: `iHatePDF`'s Node/Express backend exposes `GET /api/status` returning `{"status":"ok","timestamp":"..."}` and SPA HTML at `/`. On macOS, Control Center (`AirTunes`) binds port `5000` and Node `reusePort: true` throws `ENOTSUP`.
Decision: Expose `GET /api/status` with matching JSON schema (`status`, ISO-8601 UTC millisecond `timestamp`) alongside `GET /health`, `GET /ready`, `GET /api/tools`, and `POST /api/tools/{tool}`. Record live `../iHatePDF` responses via `rust-anything app record` (using a temporary staging harness in `/tmp` with `PORT` override and `reusePort: false` while keeping `../iHatePDF` completely untouched) and prove 100% parity via `rust-anything app replay`, supplemented by `tools/smoke.sh` (real PDF/image/resume + Argon2id auth roundtrips) and headless Chromium `tools/e2e.mjs`.
Consequences: Both `rust-anything` ground-truth HTTP replay and end-to-end PDF/UI verification pass deterministically on macOS without modifying `../iHatePDF`.

## ADR-005: Pure-Rust dependencies & zero-warning quality gate
Status: accepted (2026-11-26)
Context: `520pdf` already compiles with `lopdf 0.47`, `mozjpeg 0.10`, `image 0.25`, `zip 2`, `axum 0.8`.
Decision: Add `pdf-writer 0.15`, `chrono 0.4` (`clock`, `std`), and `tower-http 0.6` (`fs`, `compression-gzip`, `compression-br`, `set-header`). Enforce `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, and `(cd web && bun run typecheck && bun run build)`.
Consequences: Fast offline builds, small release binary, zero lint warnings.

## ADR-006: Benchmarking, Size Comparison & Visual Verification
Status: accepted (2026-11-26)
Context: Matches the verification and documentation standards established in `pulsar` and `saturn`.
Decision: Measure `../iHatePDF` (Express + React Vite build) vs `520pdf` (Axum `--release` + React Router 8 SPA) via `tools/bench.sh` (`oha`, RSS memory, startup time, LOC, dependencies, disk footprint) and capture retina headless Chromium screenshots (`docs/screenshots/*.png`) via `tools/e2e.mjs` with zero console/network errors.
Consequences: All metrics and screenshots in `README.md` are directly observed from the local machine.

## ADR-007: Argon2id authentication and REA shipped-artifact verification
Status: accepted (2026-11-26)
Context: Optional workspace authentication and webhook documentation login (`src/http/webhook.rs`) must avoid plaintext credential handling or legacy weak hashes, and release artifacts must pass REA binary/bundle inspection.
Decision:
- Hash and verify all login credentials strictly using **Argon2id** (`argon2 0.5`, `Algorithm::Argon2id`, `Version::V0x13`, default OWASP parameters) in `src/http/auth.rs`, paired with constant-time comparison and HMAC-SHA256 `HttpOnly` session cookies (`520pdf_session`).
- Add `pdf520 hash-password` CLI helper, `GET /api/auth/me`, `POST /api/auth/login`, `POST /api/auth/logout`, and React Router `/login` route (`web/app/routes/login.tsx`), keeping open-access mode active when `AUTH_USER` is unset (`iHatePDF` zero-account parity).
- Record REA `inspect_binary_hardening` and `audit_release_exposure` Evidence IDs for `target/release/pdf520` and `web/build/client` in `docs/REA-AUDIT.md`.
Consequences: No plaintext password storage or verification in memory; zero release exposure findings reported by REA.

