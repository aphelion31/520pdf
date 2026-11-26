# Handoff: `iHatePDF` -> `520pdf` (Axum 0.8 + React Router 8 + Argon2id)

## Summary
- Workspace renamed cleanly in place from `520pdf-axum-fullstack-app/` to `/Users/beyondb/Halim/Projects/CEG/prjtss/520pdf/` while keeping Cargo crate/binary name `pdf520` (`version = "0.2.0"`).
- Reference upstream `https://github.com/cu-sanjay/iHatePDF` cloned read-only to `/Users/beyondb/Halim/Projects/CEG/prjtss/iHatePDF` (git working tree clean) with attribution preserved in `LICENSE-IHATEPDF` and `README.md`.
- Node/Express server (`dist/index.js`, `GET /api/status` + Vite static bundle) replaced by single-binary Rust Axum 0.8 (`pdf520 serve`) serving the React Router 8 SPA (`web/build/client`) with dynamic compression (`tower-http`).
- Pure-Rust local engines implemented for `resume-builder` & `latex-resume` (`pdf-writer 0.15`), `compress-image` (`image 0.25` + `mozjpeg`), visual `protect` + metadata strip (`lopdf`), `pdf-to-image` (with Ghostscript fallback), plus all 16 existing PDF/OCR/office/AI workbench tools (**28 tools total**).
- Login and webhook documentation authentication upgraded to strict **Argon2id** (`Algorithm::Argon2id`, `Version::V0x13` in `src/http/auth.rs` & `src/http/webhook.rs`) with `HMAC-SHA256` signed `HttpOnly` `520pdf_session` cookies and `pdf520 hash-password` CLI (`AUTH_USER` unset keeps `iHatePDF` open-access ergonomics).

## Verification Ledger
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: 0 warnings.
- `cargo test --locked`: 36 unit tests passed (1 ignored offline mock test), `ai_tools` 1 passed (1 ignored).
- `(cd web && bun run typecheck && bun run build)`: passed (`web/build/client` = `4,789,218` bytes).
- `rust-anything app replay` against recorded `iHatePDF` `/api/status`: `1/1` (`Every case matches`, `rust/app-cases/replay-report.md`).
- `tools/smoke.sh`: passed all endpoints including `/health`, `/ready`, `/api/status`, `/api/tools` (28 tools), 15 SPA routes, PDF/image/resume tool pipelines, and full Argon2id `401 -> login -> 200` roundtrip.
- `tools/e2e.mjs`: captured all 8 retina screenshots (`docs/screenshots/*.png`) in clean Playwright headless Chromium with 0 console errors, 0 page errors, 0 failed network requests, and mobile viewport check.
- `tools/bench.sh ../iHatePDF`: measured exact footprint and `oha` throughput/latency (`85,628 req/s` vs `17,403 req/s` on `/api/status`, `7.61 MiB` vs `81.94 MiB` idle RSS, `172 ms` vs `630 ms` cold start).
- REA shipped-artifact audits (`docs/REA-AUDIT.md`): `inspect_binary_hardening` and `audit_release_exposure` (`exposed: 0`) recorded for `target/release/pdf520` and rebuilt `web/build/client`.

## Cutover & Operations
- Run `rust/cutover.sh` (or `--bench` / `--e2e`) to build, test, and run live cutover smoke verification against `127.0.0.1:8089`.
- Legacy `static/` directory was removed after confirming zero remaining references across Rust source, SPA, tests, and Docker configs.
