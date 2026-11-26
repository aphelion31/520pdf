# Domain Glossary (`iHatePDF` -> `520pdf`)

| `iHatePDF` Concept / Symbol | Meaning in Upstream | `520pdf` Equivalent |
| --- | --- | --- |
| `GET /api/status` | Express health endpoint `{ status, timestamp }` | `src/http/workbench.rs::api_status` |
| `FileVault` (`client/src/context/FileVault.tsx`) | Shared in-memory multi-file tray persisted across routes | `web/app/context/file-vault.tsx::FileVaultProvider` |
| `DropHero` (`client/src/components/DropHero.tsx`) | Landing drag-and-drop zone with file chips & smart action links | `web/app/components/DropHero.tsx` + `web/app/routes/drop.tsx` (`/drop`) |
| `pdfUtils.mergePdfFiles` | Combine multiple PDFs in order | `POST /api/tools/merge` (`src/features/pdf.rs`) + `/merge-pdfs` |
| `pdfUtils.splitPdf` | Extract pages or split into individual PDFs | `POST /api/tools/split` (`src/features/pdf.rs`) + `/split-pdf` |
| `pdfUtils.compressPdf` | Low/medium/high presets + target KB compression | `POST /api/tools/compress` (`src/main.rs`) + `/compress-pdf` |
| `pdfUtils.convertPdfToImages` | Rasterize PDF pages to PNG/JPG/WebP | `POST /api/tools/pdf-to-image` (`src/features/new_media.rs`) + `/pdf-to-image` |
| `imageUtils.convertImagesToPdf` | Images into multi-page PDF with page sizing & orientation | `POST /api/tools/image-to-pdf` (`src/features/pdf.rs`) + `/image-to-pdf` |
| `CompressImage.tsx` | Image shrink with quality, max width, format, live preview | `POST /api/tools/compress-image` (`src/features/new_media.rs`) + `/compress-image` |
| `pdfUtils.addWatermark` | Diagonal text watermark across pages | `POST /api/tools/watermark` (`src/features/pdf.rs`) + `/watermark-pdf` |
| `LockPdf` / `pdfUtils.protectPdf` | Visual watermark + border + metadata strip (or AES-256 password) | `POST /api/tools/protect` (`mode=visual` in `pdf.rs` or `qpdf` in `conversions.rs`) + `/lock-pdf` |
| `SignPdf` / `pdfUtils.signPdf` | Draw/upload/type signature + interactive placement | `POST /api/tools/sign` (`src/features/pdf.rs`) + `/sign-pdf` |
| `EquationImage` / `mathUtils` | Live KaTeX math editor, symbol palette, cropped PNG/JPG export | `web/app/routes/equation-to-image.tsx` + `src/features/equation.rs` |
| `LatexResume.tsx` | LaTeX source editor with Classic/Minimal/Academic templates -> PDF | `POST /api/tools/latex-resume` (`src/features/resume.rs`) + `/latex-resume` |
| `ResumeMaker.tsx` | Interactive form resume builder with live preview -> PDF | `POST /api/tools/resume-builder` (`src/features/resume.rs`) + `/resume-builder` |
