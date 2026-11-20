# Conversion parity status

| Tool | Acceptance criteria | Evidence / remaining gap |
|---|---|---|
| Office to PDF | Office content, page layout and embedded media preserved | LibreOffice conversion; DOCX text smoke fixture. PPT/XLS fidelity and external-link isolation not yet verified. |
| HTML to PDF | CSS page layout, no network or file resources | WeasyPrint deny-all URL fetcher; smoke checks CSS page break, hidden content, file and HTTP references. Runtime integration not yet executed. Resource limits and OS sandbox still required for untrusted documents. |
| Compare | Report textual and per-page visual differences | 72 dpi bounded raster comparison and text diff. Smoke tests both; no visual overlay or semantic diff. |
| PDF to Markdown | Preserve paragraphs, evidenced headings/tables | Extracted text only. Headings and tables not inferred. |
| Unlock | Correct password decrypts, wrong password rejected | qpdf; smoke includes protected round trip, wrong-password check. |
| Protect | 256-bit encryption, owner and user passwords | qpdf private job file; permission options not implemented. |
| PDF/A | Generate PDF/A and independently validate | OCRmyPDF PDF/A-2, smoke checks metadata only. veraPDF validation not installed or implemented; conformance unverified. |
| Scan to PDF | OCR language selection and deskew | OCRmyPDF accepts language and optional deskew. Container installs eng, ind, vie. Smoke exercises English and deskew. |
| Repair | Rebuild recoverable structure and check | MuPDF clean plus qpdf check, smoke damages xref and checks recovered text. Missing content unrecoverable. |
| Redact | Visual rectangles and irreversible removal | Rasterization with bounded page-relative top-left rectangles, rejects rotated/cropped pages. Smoke checks text, bytes and black pixels. No selective content-preserving redaction. |

Tests in `tests/conversions-smoke.sh` require the runtime image and service. Local machine lacks Docker daemon and runtime binaries, so end-to-end results are not claimed. HTML resource fetching is intentionally disabled, including embedded assets and fonts.
