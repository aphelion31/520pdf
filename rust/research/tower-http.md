# Research: `tower-http` (`0.6`)

- Primary source: `docs.rs/tower-http/0.6` (`fs`, `compression-gzip`, `compression-br`, `set-header`).
- SPA serving pattern in Axum 0.8 (`pulsar/src/main.rs` & `saturn/src/main.rs`):
  `ServeDir::new(&web_dir).fallback(ServeFile::new(web_dir.join("index.html")))`
- Semantic traps:
  - API routes (`/health`, `/ready`, `/api/status`, `/api/tools`, `/api/tools/{tool}`) must be matched BEFORE `.fallback_service(spa)` so `/api/*` never serves `index.html` on 404/405.
  - Wrap router in `CompressionLayer::new()` for gzip/br compressed JS/CSS bundles in production.
