# Independent reference inspection

Public reference: https://www.ilovepdf.com/ (20 November 2026).
REA 6.9.0 inspected an isolated Chromium CDP target using `analyze-web-bundle`.
Evidence ID: `ev_7efd22a011f385f633ec82ffe8999068cd86552ab7489777225e7aabe97950d8`.
Eight observed scripts were examined. Server-only behavior and complete route coverage
were unknown. No private API, account, document upload or proprietary backend was accessed.

Visible workflow retained: compact navigation, searchable tool directory, category filters,
large tool selection cards, focused upload/options screen, downloadable result.
520pdf uses its own brand, copy, SVG icons and Basecoat components. No reference-site
source, images, logos or JavaScript are shipped in the application.

REA is a development analysis tool, not a Rust web framework or production dependency.
