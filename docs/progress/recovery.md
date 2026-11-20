# Conversion acceptance recovery

2026-11-20: root recovered failed worker changes. Local Rust suite passes (23 binary tests, one integration test, runtime-dependent tests ignored locally). Isolated Ubuntu runtime independently passed HTML CSS/two pages, veraPDF PDF/A-2b compliance, encryption extraction denial, unlock, compare, Markdown text, OCR invocation, repair and irreversible text removal.

Integrated improvements: layout-derived Markdown headings and aligned tables, validated permission options, mandatory distinct owner password, fail-closed veraPDF validation. Markdown layout heuristics are not general semantic reconstruction.

Real multilingual AI regression passed with mock provider, Vietnamese/Japanese extraction and preserved page boundaries. Fixed LibreOffice ignoring page breaks on div elements by using paragraphs. No paid provider calls.

Expanded Office fixtures require explicit flat OpenDocument MIME types and export filters. PPTX text round trip passes. Spreadsheet text fixture shortened to fit default column width. Full expanded smoke passed in isolated Ubuntu container: DOCX/PPTX/XLSX text round trips, HTML CSS and resource rejection, visual/text comparison, Markdown headings/tables, encryption permissions and wrong password rejection, unlock, archival conversion, real scanned-page OCR, damaged-xref repair, black-pixel/text/byte redaction checks, AI consent rejection. Iframe fallback text is harmless and permitted, external iframe resources remain blocked.

Remaining limits: Office external-link isolation not independently verified, HTML assets deliberately blocked, no semantic comparison overlay, raster redaction discards selectable content. Do not claim full iLovePDF parity. Production service unchanged during recovery.
