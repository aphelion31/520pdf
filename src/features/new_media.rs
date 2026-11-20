//! Bounded image compression and PDF rasterization.
use image::{ImageFormat, ImageReader, codecs::jpeg::JpegEncoder};
use std::{
    collections::HashMap,
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

fn compress(
    input: &Path,
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    let target = options
        .get("target_kb")
        .map(String::as_str)
        .unwrap_or("250")
        .parse::<u64>()
        .map_err(|_| "target_kb must be an integer between 1 and 100000")?;
    if !(1..=100_000).contains(&target) {
        return Err("target_kb must be an integer between 1 and 100000".into());
    }
    let bytes = fs::read(input).map_err(|e| e.to_string())?;
    let format = image::guess_format(&bytes).map_err(|e| format!("Invalid image: {e}"))?;
    let (ext, mime) = match format {
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
    let img = reader
        .decode()
        .map_err(|e| format!("Invalid or oversized image: {e}"))?;
    if u64::from(img.width()) * u64::from(img.height()) > 40_000_000 {
        return Err("Image exceeds 40 million pixels".into());
    }
    // Do not discard transparency or change a non-JPEG image's format.
    let mut best = bytes;
    if format == ImageFormat::Jpeg && best.len() as u64 > target * 1000 {
        let original_len = best.len();
        let rgb = img.to_rgb8();
        let mut low = 1u8;
        let mut high = 95u8;
        while low <= high {
            let quality = low + (high - low) / 2;
            let mut encoded = Vec::new();
            JpegEncoder::new_with_quality(&mut encoded, quality)
                .encode_image(&rgb)
                .map_err(|e| e.to_string())?;
            let fits = encoded.len() as u64 <= target * 1000;
            if fits {
                if encoded.len() < original_len {
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
    let out = dir.join(format!("compressed.{ext}"));
    fs::write(&out, best).map_err(|e| e.to_string())?;
    Ok((out, mime.into()))
}

fn rasterize(
    input: &Path,
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    let dpi = options
        .get("dpi")
        .map(String::as_str)
        .unwrap_or("150")
        .parse::<u32>()
        .map_err(|_| "dpi must be between 36 and 300")?;
    if !(36..=300).contains(&dpi) {
        return Err("dpi must be between 36 and 300".into());
    }
    let format = options.get("format").map(String::as_str).unwrap_or("png");
    let (flag, ext) = match format {
        "png" => ("-png", "png"),
        "jpg" | "jpeg" => ("-jpeg", "jpg"),
        _ => return Err("format must be png or jpeg".into()),
    };
    let doc = lopdf::Document::load(input).map_err(|e| format!("Invalid PDF: {e}"))?;
    let count = doc.get_pages().len();
    if count == 0 || count > 50 {
        return Err("PDF must contain 1 to 50 pages".into());
    }
    let input = input.canonicalize().map_err(|e| e.to_string())?;
    let out = dir.join("pages.zip");
    let file = fs::File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    for page in 1..=count {
        let prefix = dir.join(format!("render-{page}"));
        let prefix = if prefix.is_absolute() {
            prefix
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(prefix)
        };
        let mut child = Command::new("pdftoppm")
            .args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-singlefile",
                "-r",
                &dpi.to_string(),
                "-scale-to",
                "3000",
                flag,
            ])
            .arg(&input)
            .arg(&prefix)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start pdftoppm: {e}"))?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err("PDF rendering failed".into());
                }
                break;
            }
            if start.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("PDF rendering timed out".into());
            }
            thread::sleep(Duration::from_millis(25));
        }
        let rendered = prefix.with_extension(ext);
        let data = fs::read(&rendered).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(rendered);
        zip.start_file(
            format!("page-{page:03}.{ext}"),
            SimpleFileOptions::default(),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
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
            &[input.clone()],
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
}
