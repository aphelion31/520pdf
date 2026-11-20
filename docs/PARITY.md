# 23-tool parity ledger

Each tool progresses through: observed baseline, implemented upgrade, automated check,
real file acceptance, visual workflow acceptance. An endpoint alone is not parity.
No feature is labelled equivalent without a concrete acceptance criterion.

| # | Tool | Current upgrade status | Evidence |
|---|---|---|---|
| 01 | Merge PDF | In progress | Native engine worker |
| 02 | Split PDF | ZIP all/ranges real HTTP passed | 12 outputs and 2 range outputs, native tests |
| 03 | Organize PDF | In progress | Native + visual workers |
| 04 | Compress PDF | Lossless-fit real HTTP passed, quality parity open | Byte-identical output, OK/OVER_TARGET headers |
| 05 | Images to PDF | In progress | Native engine worker |
| 06 | Office to PDF | In progress | Conversion worker |
| 07 | Crop PDF | In progress | Native + visual workers |
| 08 | Compare PDF | In progress | Conversion worker |
| 09 | PDF to Markdown | In progress | Conversion worker |
| 10 | Sign PDF | In progress | Native + visual workers |
| 11 | Watermark PDF | In progress | Native + visual workers |
| 12 | Rotate PDF | In progress | Native + visual workers |
| 13 | HTML to PDF | In progress | Conversion worker |
| 14 | Unlock PDF | In progress | Conversion worker |
| 15 | Protect PDF | In progress | Conversion worker |
| 16 | PDF to PDF/A | In progress | Conversion worker |
| 17 | Scan to PDF | In progress | Conversion worker |
| 18 | Page numbers | In progress | Native + visual workers |
| 19 | Repair PDF | In progress | Conversion worker |
| 20 | PDF forms | In progress | Native engine worker |
| 21 | Redact PDF | In progress | Conversion + visual workers |
| 22 | AI summarize | In progress | AI worker, mock provider only |
| 23 | Translate PDF | In progress | AI worker, mock provider only |

Worker configuration verified from all four session metadata files: `9router:cx/gpt-6.1-terra`, effort `medium`.
Per-feature detailed reports live in `docs/progress/`.
