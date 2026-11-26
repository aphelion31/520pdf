# Research: `pdf-writer` (`0.15`)

- Primary source: `docs.rs/pdf-writer/0.15` + `saturn/src/live_http.rs` (confirmed working on Cargo `0.15`).
- Pure-Rust zero-C-dep library generating PDF 1.7 bytes into `Vec<u8>`.
- A4 portrait size: `595.28 x 841.89` PDF points (72 pt = 1 inch).
- Base14 Type1 fonts (`Helvetica`, `Helvetica-Bold`, `Helvetica-Oblique`, `Courier`) use `WinAnsiEncoding` (`type1.encoding().win_ansi()`).
- Semantic traps:
  - Map em-dash/en-dash/bullet/middot/smart quotes or non-Latin1 Unicode safely to WinAnsi byte values (or ASCII fallbacks) before calling `Str(&bytes)`.
  - Track object `Ref` allocation cleanly for Catalog, Pages, Page(s), Content stream(s), and Font(s), then `pdf.finish()`.
