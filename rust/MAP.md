# Whole-App Port Map (`iHatePDF` -> `520pdf`)

Goal: Complete the port of `cu-sanjay/iHatePDF` into single-binary `520pdf` (Axum 0.8 backend + React Router 8 SPA in `web/`), verified with `rust-anything app record` / `app replay`, end-to-end smoke/Chromium tests, REA release inspection, benchmarks, and retina screenshots.

- Decisions index: [DECISIONS.md](DECISIONS.md) (`ADR-001` .. `ADR-007`)
- Domain glossary: [GLOSSARY.md](GLOSSARY.md)
- Crate research: [research/](research/)
- REA security & release audit: [../docs/REA-AUDIT.md](../docs/REA-AUDIT.md)
- Final handoff: [HANDOFF.md](HANDOFF.md)
- Cutover verification script: [cutover.sh](cutover.sh)

## Wave Status

| Wave | Unit | Target files | State | Notes |
| --- | --- | --- | --- | --- |
| Wave 1 | Rename repo, clone read-only `../iHatePDF`, `rust-anything analyze`, tracking docs | `rust/{DECISIONS,MAP,GLOSSARY}.md`, `rust/research/`, `.gitignore`, `LICENSE-IHATEPDF` | `done` | Workspace renamed `520pdf-axum-fullstack-app` -> `520pdf` |
| Wave 2 | Axum backend, Argon2id auth, and pure-Rust engines (`pdf-writer`, `new_media`, visual `protect`, `compress`) | `Cargo.toml`, `src/features/*.rs`, `src/http/{auth,webhook,workbench}.rs`, `src/main.rs` | `done` | Pure-Rust local engines for 12/12 iHatePDF tools + Argon2id cookie/docs auth |
| Wave 3 | React Router 8 + Tailwind v4 SPA (`web/`) built with Bun 1.4.4 | `web/app/**`, `web/public/**`, `web/package.json` | `done` | 12/12 iHatePDF routes + `/login` + dynamic workbench for 16 extra tools |
| Wave 4 | Ground-truth `app record`/`app replay`, `tools/smoke.sh`, `tools/e2e.mjs`, REA audit & screenshots | `rust/app-cases/`, `tools/{smoke.sh,e2e.mjs}`, `docs/REA-AUDIT.md`, `docs/screenshots/` | `done` | 1/1 HTTP replay match, 8 retina screenshots, 0 console/network errors, REA `exposed: 0` |
| Wave 5 | Original vs Rewrite metrics, `oha` benchmark, Dockerfile, `README.md`, cleanup & commit | `tools/bench.sh`, `docker/axum/Dockerfile`, `README.md`, `rust/{HANDOFF.md,cutover.sh}` | `done` | Legacy `static/` removed; README matches `pulsar`/`saturn` format |

## Out of Scope (`ADR-002`, `ADR-003`)
- Executing arbitrary host `pdflatex` / `xelatex` binaries on untrusted LaTeX input
- Express session/Postgres user schema (`server/storage.ts`, `shared/schema.ts`, dead template boilerplate unused by `iHatePDF` routes)
- Rewriting optional Docker external engines (`soffice`, `ocrmypdf`, `verapdf`, `mutool`) into pure Rust
