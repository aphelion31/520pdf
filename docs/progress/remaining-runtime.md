# Remaining tool runtime acceptance

2026-11-20, isolated Ubuntu container `pdf520-parity-recovered`, image `520pdf-parity:0.2.1`. Production unchanged. Generated fixtures only, no user documents or paid provider requests.

`tests/native-smoke.sh` passed all assertions via real HTTP:

| Tool | Checked output |
|---|---|
| Merge | Four pages from two two-page inputs, qpdf structural check |
| Split | Selected second page, BETA text, out-of-range rejection |
| Organize | Reorder and duplicate to three pages, first-page text |
| Rotate | First page rotation 90 degrees |
| Crop | CropBox differs from MediaBox |
| Watermark | Added watermark text extracted |
| Page numbers | Second-page formatted number extracted |
| Sign | Visible typed signature text extracted, not cryptographic signing |
| Image to PDF | One valid PDF page with embedded raster image |
| Forms | Discover field and fill named text field, saved value checked |
| Compress | Already-fitting input byte-identical, two pages preserved |

These eleven tools supplement ten conversion tools previously verified by `tests/conversions-smoke.sh`. Translation has real PDF rendering/extraction verification with local mock provider. Summary positive-path runtime regression also passed: real PDF extraction, local mock provider response and exact saved summary text. All 23 tools now have backend positive-path fixture evidence: 21 via HTTP, two AI workflows via direct runtime integration with mock provider. AI HTTP consent rejection is covered separately.

Scope: backend fixture acceptance, not browser click-through or full commercial feature parity. Compression test covers the no-recompression path, not quality or size reduction on large files. Native visual fidelity, complex form variants and unsupported document features retain documented limits in `native.md`.
