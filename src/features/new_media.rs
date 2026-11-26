//! Bounded image compression and PDF rasterization.
use image::{
    DynamicImage, ImageFormat, ImageReader, codecs::jpeg::JpegEncoder, imageops::FilterType,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zip::{ZipWriter, write::SimpleFileOptions};

pub fn run(
    tool: &str,
    files: &[PathBuf],
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    if files.len() != 1 {
        return Err("Upload exactly one file".into());
    }
    if fs::metadata(&files[0]).map_err(|e| e.to_string())?.len() > 50_000_000 {
        return Err("Input exceeds 50 MB".into());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    match tool {
        "compress-image" => compress(&files[0], options, dir),
        "pdf-to-image" => rasterize(&files[0], options, dir),
        _ => Err("Unknown media tool".into()),
    }
}

fn flatten_on_white(img: &DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
    for p in rgba.pixels() {
        let a = u32::from(p.0[3]);
        for c in &p.0[..3] {
            rgb.push(((u32::from(*c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    image::RgbImage::from_raw(w, h, rgb).expect("valid rgb dimensions")
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = flatten_on_white(img);
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, quality)
        .encode_image(&rgb)
        .map_err(|e| e.to_string())?;
    Ok(encoded)
}

fn encode_format(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    let mut encoded = Cursor::new(Vec::new());
    img.write_to(&mut encoded, format)
        .map_err(|e| e.to_string())?;
    Ok(encoded.into_inner())
}

fn compress(
    input: &Path,
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    let target = options
        .get("target_kb")
        .filter(|s| !s.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("250")
        .parse::<u64>()
        .map_err(|_| "target_kb must be an integer between 1 and 100000")?;
    if !(1..=100_000).contains(&target) {
        return Err("target_kb must be an integer between 1 and 100000".into());
    }

    let explicit_quality = match options.get("quality").filter(|s| !s.trim().is_empty()) {
        Some(v) => {
            let q = v
                .parse::<u8>()
                .map_err(|_| "quality must be an integer between 1 and 100")?;
            if !(1..=100).contains(&q) {
                return Err("quality must be an integer between 1 and 100".into());
            }
            Some(q)
        }
        None => None,
    };

    let max_width = match options.get("max_width").filter(|s| !s.trim().is_empty()) {
        Some(v) => {
            let w = v
                .parse::<u32>()
                .map_err(|_| "max_width must be an integer between 16 and 12000")?;
            if !(16..=12000).contains(&w) {
                return Err("max_width must be an integer between 16 and 12000".into());
            }
            Some(w)
        }
        None => None,
    };

    let output_format = options
        .get("output_format")
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "auto".into());
    if !matches!(
        output_format.as_str(),
        "auto" | "jpeg" | "jpg" | "png" | "webp"
    ) {
        return Err("output_format must be auto, jpeg, png or webp".into());
    }

    let bytes = fs::read(input).map_err(|e| e.to_string())?;
    let format = image::guess_format(&bytes).map_err(|e| format!("Invalid image: {e}"))?;
    let src_ext_mime = match format {
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::WebP => ("webp", "image/webp"),
        _ => return Err("Supported images: JPEG, PNG, WebP".into()),
    };

    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(12000);
    limits.max_image_height = Some(12000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut img = reader
        .decode()
        .map_err(|e| format!("Invalid or oversized image: {e}"))?;
    if u64::from(img.width()) * u64::from(img.height()) > 40_000_000 {
        return Err("Image exceeds 40 million pixels".into());
    }

    let mut transformed = false;
    if let Some(mw) = max_width
        && img.width() > mw
    {
        let new_h =
            ((u64::from(img.height()) * u64::from(mw)) / u64::from(img.width())).max(1) as u32;
        img = img.resize_exact(mw, new_h, FilterType::Lanczos3);
        transformed = true;
    }

    let target_format = match output_format.as_str() {
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        _ => format,
    };

    // Preserve lossless PNG/WebP unchanged when only target_kb is specified without quality/resize/format override.
    if !transformed
        && explicit_quality.is_none()
        && target_format == format
        && format != ImageFormat::Jpeg
    {
        let out = dir.join(format!("compressed.{}", src_ext_mime.0));
        fs::write(&out, &bytes).map_err(|e| e.to_string())?;
        return Ok((out, src_ext_mime.1.into()));
    }

    let (out_bytes, ext, mime) = match target_format {
        ImageFormat::Jpeg => {
            if let Some(q) = explicit_quality {
                let encoded = encode_jpeg(&img, q)?;
                let final_bytes =
                    if !transformed && target_format == format && encoded.len() >= bytes.len() {
                        bytes
                    } else {
                        encoded
                    };
                (final_bytes, "jpg", "image/jpeg")
            } else {
                let mut best = if !transformed && format == ImageFormat::Jpeg {
                    bytes.clone()
                } else {
                    encode_jpeg(&img, 92)?
                };
                if best.len() as u64 > target * 1000 {
                    let baseline = best.len();
                    let mut low = 1u8;
                    let mut high = 95u8;
                    while low <= high {
                        let quality = low + (high - low) / 2;
                        let encoded = encode_jpeg(&img, quality)?;
                        let fits = encoded.len() as u64 <= target * 1000;
                        if fits {
                            if encoded.len() < baseline {
                                best = encoded;
                            }
                            low = quality + 1;
                        } else {
                            if best.len() as u64 > target * 1000 && encoded.len() < best.len() {
                                best = encoded;
                            }
                            if quality == 1 {
                                break;
                            }
                            high = quality - 1;
                        }
                    }
                }
                (best, "jpg", "image/jpeg")
            }
        }
        ImageFormat::WebP => (
            encode_format(&img, ImageFormat::WebP)?,
            "webp",
            "image/webp",
        ),
        ImageFormat::Png => (encode_format(&img, ImageFormat::Png)?, "png", "image/png"),
        _ => return Err("Supported output formats: jpeg, png, webp".into()),
    };

    let out = dir.join(format!("compressed.{ext}"));
    fs::write(&out, out_bytes).map_err(|e| e.to_string())?;
    Ok((out, mime.into()))
}

fn parse_pages(spec: Option<&str>, count: usize) -> Result<Vec<usize>, String> {
    let Some(raw) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok((1..=count).collect());
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for part in raw.split(',') {
        let part = part.trim();
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let a = a
            .trim()
            .parse::<usize>()
            .map_err(|_| "Invalid page selection")?;
        let b = b
            .trim()
            .parse::<usize>()
            .map_err(|_| "Invalid page selection")?;
        if a == 0 || a > b || b > count {
            return Err(format!("Page selection must be within 1-{count}"));
        }
        for n in a..=b {
            if seen.insert(n) {
                out.push(n);
            }
        }
    }
    if out.is_empty() {
        return Err("Select at least one page".into());
    }
    Ok(out)
}

fn run_timed(mut cmd: Command, label: &str) -> Result<(), String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start {label}: {e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err(format!("{label} rendering failed"));
            }
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{label} rendering timed out"));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn has_binary(name: &str) -> bool {
    Command::new(name)
        .arg("-v")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn rasterize(
    input: &Path,
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    let dpi = options
        .get("dpi")
        .filter(|s| !s.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("150")
        .parse::<u32>()
        .map_err(|_| "dpi must be between 36 and 300")?;
    if !(36..=300).contains(&dpi) {
        return Err("dpi must be between 36 and 300".into());
    }
    let format = options
        .get("format")
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "png".into());
    let (ppm_flag, gs_device, raw_ext, out_ext) = match format.as_str() {
        "png" => ("-png", "png16m", "png", "png"),
        "jpg" | "jpeg" => ("-jpeg", "jpeg", "jpg", "jpg"),
        "webp" => ("-png", "png16m", "png", "webp"),
        _ => return Err("format must be png, jpeg, or webp".into()),
    };
    let doc = lopdf::Document::load(input).map_err(|e| format!("Invalid PDF: {e}"))?;
    let count = doc.get_pages().len();
    if count == 0 || count > 50 {
        return Err("PDF must contain 1 to 50 pages".into());
    }
    let selected = parse_pages(options.get("pages").map(String::as_str), count)?;
    let input = input.canonicalize().map_err(|e| e.to_string())?;
    let out = dir.join("pages.zip");
    let file = fs::File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let use_pdftoppm = has_binary("pdftoppm");

    for page in selected {
        let prefix = dir.join(format!("render-{page}"));
        let prefix = if prefix.is_absolute() {
            prefix
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(prefix)
        };
        let rendered = prefix.with_extension(raw_ext);
        if use_pdftoppm {
            let mut cmd = Command::new("pdftoppm");
            cmd.args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-singlefile",
                "-r",
                &dpi.to_string(),
                "-scale-to",
                "3000",
                ppm_flag,
            ])
            .arg(&input)
            .arg(&prefix);
            run_timed(cmd, "pdftoppm")?;
        } else {
            let mut cmd = Command::new("gs");
            cmd.args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                &format!("-sDEVICE={gs_device}"),
                &format!("-r{dpi}"),
                &format!("-dFirstPage={page}"),
                &format!("-dLastPage={page}"),
                &format!("-o{}", rendered.display()),
                "-f",
            ])
            .arg(&input);
            run_timed(cmd, "ghostscript")?;
        }

        let raw_data = fs::read(&rendered).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&rendered);
        let final_data = if out_ext == "webp" {
            let img = image::load_from_memory(&raw_data).map_err(|e| e.to_string())?;
            encode_format(&img, ImageFormat::WebP)?
        } else {
            raw_data
        };
        zip.start_file(
            format!("page-{page:03}.{out_ext}"),
            SimpleFileOptions::default(),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&final_data).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok((out, "application/zip".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_png_and_validates_options() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        image::RgbaImage::from_pixel(10, 10, image::Rgba([1, 2, 3, 0]))
            .save(&input)
            .unwrap();
        let original = fs::read(&input).unwrap();
        let (output, mime) = run(
            "compress-image",
            std::slice::from_ref(&input),
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(fs::read(output).unwrap(), original);
        let mut options = HashMap::new();
        options.insert("target_kb".into(), "0".into());
        assert!(run("compress-image", &[input], &options, dir.path()).is_err());
    }

    #[test]
    fn resizes_and_converts_image_formats() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        let mut img = image::RgbaImage::new(200, 100);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgba([(x % 255) as u8, (y % 255) as u8, 120, 255]);
        }
        img.save(&input).unwrap();

        let opts = HashMap::from([
            ("max_width".into(), "80".into()),
            ("quality".into(), "75".into()),
            ("output_format".into(), "jpeg".into()),
        ]);
        let (out, mime) = run(
            "compress-image",
            std::slice::from_ref(&input),
            &opts,
            dir.path(),
        )
        .unwrap();
        assert_eq!(mime, "image/jpeg");
        let decoded = image::open(out).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (80, 40));

        let webp_opts = HashMap::from([
            ("max_width".into(), "60".into()),
            ("output_format".into(), "webp".into()),
        ]);
        let (out_webp, mime_webp) =
            run("compress-image", &[input], &webp_opts, dir.path()).unwrap();
        assert_eq!(mime_webp, "image/webp");
        let decoded_webp = image::open(out_webp).unwrap();
        assert_eq!((decoded_webp.width(), decoded_webp.height()), (60, 30));
    }

    #[test]
    fn parses_page_ranges() {
        assert_eq!(parse_pages(None, 4).unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(parse_pages(Some("1,3-4"), 5).unwrap(), vec![1, 3, 4]);
        assert!(parse_pages(Some("6"), 5).is_err());
    }
}
