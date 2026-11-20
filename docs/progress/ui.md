# UI progress

PDF.js 4.10.38 is vendored with its Apache 2.0 license. PDF previews are local, with PDF.js evaluation and XFA disabled. Processing still uploads to the configured 520pdf server, not a third-party preview service.

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Catalog and all 23 tools | Existing catalog-driven forms retained | Browser mock regression covers search, category, upload, errors, download and mobile overflow. Non-PDF inputs have no thumbnail preview. |
| PDF preview | Implemented | Locally renders up to 100 pages. Preview failures leave manual fields available. |
| Organize | Implemented | Drag reorder, keyboard-accessible earlier/later buttons, include checkboxes; sends pages string. |
| Split | Implemented | Include checkboxes and pages string; no automatic multi-file output. |
| Rotate | Implemented | Per-page rotation sends rotations JSON. |
| Crop | Implemented | Page selector, drag rectangle and numeric PDF point fields. Selected page sent as `pages`. |
| Redact | Implemented, runtime test blocked | Multiple areas on selectable pages, with add/remove and numeric alternatives. Local integration returned missing `pdftoppm`; independently verify redacted output before sharing sensitive documents. |
| Sign | Implemented | Draw or upload PNG/JPEG, preview actual image on page 1, set numeric position and size, send `image_base64`. |
| Watermark and page numbers | Partial | Text and position preview for page 1; exact font metrics and rotation are approximate. |
| Remaining tools | Existing form workflows | PDF inputs have local previews; tool-specific behavior depends on server. No claim of full visual parity. |

Integrated follow-up: Brave selected crop page 2 and downloaded `crop.pdf`; redaction UI submitted two page-specific rectangles but backend returned missing `pdftoppm`; drawn signature placement produced `sign.pdf`. Browser mock regression also tests rectangle removal and signature position. No browser errors.
