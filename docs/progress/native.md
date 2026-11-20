# Native tool progress

Reference pages inspected 2026-11-20: [split](https://www.ilovepdf.com/split_pdf), [images](https://www.ilovepdf.com/jpg_to_pdf), [watermark](https://www.ilovepdf.com/pdf_add_watermark), [page numbers](https://www.ilovepdf.com/add_pdf_page_number). These are feature references, not claims of parity.

| Feature | Implemented acceptance criteria and evidence | Remaining gaps |
| --- | --- | --- |
| Merge | Multiple PDFs, page order and inherited resources preserved. `operations_roundtrip_real_files` | Interactive forms rejected, outlines discarded. |
| Split | `pages=1,3-5` creates one PDF. `mode=all` produces a ZIP per page, `mode=ranges` with `ranges=1-3;5` produces a ZIP per range, max 500 outputs. `operations_roundtrip_real_files`, `split_zip_duplicate_and_image_watermark` | Fixed-size splitting not implemented. |
| Organize | Reorder/remove and duplicate unannotated pages, inherited boxes preserved. `reorder_preserves_inherited_boxes`, `split_zip_duplicate_and_image_watermark` | Duplicating annotated pages is rejected. |
| Image to PDF | PNG/JPEG/WebP, alpha flattened white, `page_size=fit|a4|letter`, `orientation=portrait|landscape`, numeric `margin` points. `image_to_pdf_embeds_real_pixels`, `advanced_native_options_roundtrip` | Fit size follows image geometry and does not enforce orientation, no DPI or additional paper sizes. |
| Crop | Selected page rectangular CropBox or per-edge `margin_left`, `margin_right`, `margin_top`, `margin_bottom`, checked against MediaBox, PDF bottom-left coordinates. `operations_roundtrip_real_files`, `advanced_native_options_roundtrip` | Does not permanently remove content. Rotated-page visual coordinates not transformed. |
| Sign | Typed ASCII or `image_base64` with alpha, `x`, `y`, `width`, `height`, `pages`. `operations_roundtrip_real_files`, `advanced_native_options_roundtrip` | Visible mark only, not a cryptographic signature. No certificate or identity verification. |
| Watermark | ASCII text or `image_base64` with selection, `x`, `y`, `rotation` degrees, `opacity` 0..1. Text has horizontal `position` anchor. `advanced_native_options_roundtrip`, `split_zip_duplicate_and_image_watermark` | No background layer, Unicode fonts or text alignment. |
| Rotate | Selected pages and JSON `rotations` page-to-angle mapping, multiple of 90. `advanced_native_options_roundtrip` | No automatic correction. |
| Page numbers | `start`, ASCII `format` with `{n}`, `{total}`, horizontal `position` anchor, `x`, `y`; other overlay controls. `advanced_native_options_roundtrip` | Horizontal position is the text start anchor, not alignment. No facing-page mode or Unicode. |
| Forms | `action=discover` returns JSON local field names/types; `field.<name>` fills simple text and checkboxes with existing appearances. `form_value_and_appearance_are_updated`, `checkbox_and_discovery_roundtrip` | No fully qualified hierarchy, choice/radio/signature fields, complex text formatting or Unicode; discovery includes locally named field objects, not a validated full field tree. |

Validation: `cargo test tools::tests --quiet` passed on 2026-11-20. Tests use generated PDFs/images and inspect saved outputs; they do not establish visual fidelity in every PDF viewer. Preserve original PDFs: edited output may lose outline references or invalidate unsupported document features. Digitally signed and encrypted inputs are rejected.
