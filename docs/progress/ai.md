# AI tools progress

Implemented consent-gated summarize and translate in `src/ai_tools.rs`. Provider requires `AI_API_URL` (HTTPS), `AI_API_KEY`, and `AI_MODEL`. Each request sends extracted document text to that provider and may incur charges. No live provider requests were made during development.

Options: `consent=true`; translate requires `target_language`; summarize accepts `summary_length=short|medium|long` and `summary_language` (default English). `ocr=true` enables OCRmyPDF with `ocr_language` (default eng). `output_format=txt|pdf` defaults to txt. `bilingual=true` includes original and translation. AI text is limited to 120,000 UTF-8 bytes and provider output to 2 MB. PDF output uses escaped generated UTF-8 HTML and LibreOffice. Page breaks depend on provider preserving form-feed characters; translation and layout fidelity are not guaranteed. Runtime dependencies: pdftotext, LibreOffice for PDF, OCRmyPDF and language data for OCR.

Test: `cargo test --test ai_tools` uses an explicitly constructed loopback-only mock provider. Production configuration never enables insecure HTTP. This test checks consent, endpoint policy, escaping and Unicode provider response. LibreOffice and pdftotext were unavailable in the test environment, so end-to-end rendering was not verified here.

Integration: declare `mod ai_tools` and route summarize/translate through `ai_tools::run` in the application. Actual model identity could not be verified from available session metadata; environment reports runtime provider `openai`, active provider `copilot`, but exposes no model or reasoning level.
