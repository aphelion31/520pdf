# Source structure

- `src/features/pdf.rs`: native PDF operations, annotations and forms.
- `src/features/conversions.rs`: document conversion and external converter orchestration.
- `src/features/ai.rs`: summary/translation workflows and provider safety checks.
- `src/http/workbench.rs`: browser routes, tool catalog and multipart HTTP adapter.
- `src/http/webhook.rs`: compression webhook and API documentation routes.
- `src/http/docs.html`, `src/http/openapi.json`: webhook API assets.
- `src/main.rs`: executable dispatch and existing compression pipeline.
- `static/`: shared browser assets and workbench UI, unchanged by this refactor.
- `tests/`: integration and runtime acceptance checks.

Features own their implementation. HTTP adapters invoke features, not the reverse. External CLI orchestration remains alongside the workflow using it, not a pure domain layer. This is feature grouping, not a claim of complete Clean Architecture isolation.

No empty domain/application/infrastructure folders, generic repositories or shared component wrappers. Extract shared code only when multiple features need the same behavior and safety contract. Existing browser UI is vanilla JavaScript, not a component framework.

Verification: `cargo fmt --check`, `cargo test --locked`. Runtime conversion acceptance: `sh tests/conversions-smoke.sh` inside the runtime with the service listening.
