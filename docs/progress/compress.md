# Compress PDF progress

Implemented:
- Target size in decimal KB with explicit `OK` / `OVER_TARGET` response header.
- Already-fitting PDFs returned byte-for-byte unchanged, preserving original image fidelity.
- Compressed output never replaces an original with a larger result.
- Exactly one input, finite bounded target, valid nonempty PDF required.
- Original upload never overwritten. Same page-count structural check after compression.

Regression: `tests::fitting_compression_preserves_original_bytes` verifies byte equality and
invalid-input rejection. Existing compression engine uses Ghostscript + Rust/mozjpeg quality
search. Commercial preset/visual quality equivalence is not established. Signed PDFs larger
than target may have their digital signatures invalidated by compression, as with other edits.
