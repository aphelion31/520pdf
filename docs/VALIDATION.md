# Validation record

20 November 2026:

- Real Chromium UI: 23 catalog tools rendered, split upload through downloaded result passed,
  mobile viewport 390px had no horizontal overflow.
- `cargo test`: 16 passed.
- Native HTTP workflows: merge (24 output pages from two 12-page inputs), split and organize
  (one selected page), rotate, crop, watermark, page numbers and visible sign. All downloaded
  PDFs opened with Ghostscript and had expected page counts.
- Production Docker image built successfully on Linux amd64.
- Conversion smoke script passed inside that image as non-root, read-only filesystem,
  dropped capabilities, 2 GiB RAM, 2 CPUs, PID ceiling and **network disabled**.
- Real conversion checks: DOCX to PDF, sanitized HTML to PDF, compare, Markdown extraction,
  AES protection, unlock, PDF/A conversion, scan OCR, damaged-xref repair and raster redaction.
- Summarize/translate without consent rejected. No paid AI requests made.

Not validated: live AI provider output, every Office format/layout, independent veraPDF archival
certification, cryptographic signatures (not offered), visual document comparison (not offered),
and large/adversarial production load. HTML conversion is text-only. Translation returns text.
These are explicit capability boundaries, not completed parity with all commercial PDF suites.
