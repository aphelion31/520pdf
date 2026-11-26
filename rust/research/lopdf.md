# Research: `lopdf` (`0.47`)

- Primary source: `docs.rs/lopdf/0.47` and `src/features/pdf.rs`.
- Handles native PDF manipulation (`merge`, `split`, `organize`, `rotate`, `crop`, `watermark`, `sign`, `page-numbers`, `forms`, `image-to-pdf`, plus visual `protect`).
- Semantic traps:
  - Inherited page attributes (`MediaBox`, `CropBox`, `Resources`, `Rotate`) must be materialized when copying/reordering pages (`materialize()`).
  - Visual protection (`mode=visual`): clear `/Info` entries (`Title`, `Author`, `Subject`, `Keywords`), set `Producer`/`Creator` to `520pdf`, add diagonal watermark XObject and subtle border rectangle without requiring external `qpdf`.
  - Keep `qpdf` AES-256 encryption path when a user `password` is supplied.
