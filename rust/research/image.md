# Research: `image` (`0.25`) + `mozjpeg` (`0.10`)

- Primary source: `docs.rs/image/0.25` and `src/features/new_media.rs`.
- `image 0.25` supports PNG, JPEG, WebP decode and encode (`png`, `jpeg`, `webp` features already enabled in `Cargo.toml`).
- `image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Lanczos3)` preserves aspect ratio cleanly.
- Semantic traps:
  - Always enforce `image::Limits` (`max_image_width`, `max_image_height`, `max_alloc`) and total pixel bounds against decompression bombs.
  - When converting transparent RGBA images to JPEG, flatten onto white (`#ffffff`) before encoding RGB.
  - Preserve original bytes when re-encoding would grow size unless explicit resize/format conversion was requested.
