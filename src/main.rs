//! Compress a PDF under a target size (default 250 KB).
//!
//! usage: compress-pdf input.pdf [-t 250] [-o output.pdf]
//!        compress-pdf serve            (HTTP webhook, see server.rs)
//! needs: ghostscript on PATH (`brew install ghostscript`)
//!
//! Pipeline: gs downsample images to 150 dpi (lossless) -> strip TrueType hinting -> JPEG re-encode.
//! Transparent images (logos, stamps, signatures) keep 150 dpi + GFX_Q: line art at q15 looks broken.
//! Photos absorb the budget: highest photo DPI that fits with quality >= MIN_Q, else best effort.

use lopdf::{Document, Object, ObjectId, SaveOptions};
use mozjpeg::{ColorSpace, Compress};
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

mod features;
mod http;

use features::{ai as ai_tools, conversions, pdf as tools};
use http::{webhook as server, workbench as web};

const DPIS: [u32; 7] = [150, 120, 110, 100, 90, 80, 72];
const MIN_Q: u8 = 30; // ponytail: fixed ladder, tune here if docs differ
const MAX_Q: u8 = 75;
const FLOOR_Q: u8 = 15;
const MASK_Q: u8 = 60;
const GFX_Q: u8 = 60;
const GFX_PIXELS: usize = 40_000;
const GFX_DPI: u32 = 110; // ponytail: fixed; raise if logos still look soft and budget allows
const BASE_DPI: u32 = 150;
const MAX_PIXELS: usize = 100_000_000; // decompression-bomb guard

type Res<T> = Result<T, String>;

fn main() -> ExitCode {
    if std::env::args().len() == 1 || std::env::args().nth(1).as_deref() == Some("serve") {
        return web::main();
    }
    if std::env::args().nth(1).as_deref() == Some("webhook") {
        return server::main();
    }
    if std::env::args().nth(1).as_deref() == Some("hash-password") {
        let password = match std::env::args().nth(2) {
            Some(p) => p,
            None => {
                use std::io::Read;
                let mut buf = String::new();
                if std::io::stdin().read_to_string(&mut buf).is_err() {
                    eprintln!("error: failed to read password from stdin");
                    return ExitCode::FAILURE;
                }
                buf.trim_end_matches(['\r', '\n']).to_string()
            }
        };
        return match http::auth::hash_password(&password) {
            Ok(phc) => {
                println!("{phc}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Res<bool> {
    let (input, target_kb, output) = parse_args()?;
    if !input.is_file() {
        return Err(format!("not found: {}", input.display()));
    }
    if Command::new("gs").arg("--version").output().is_err() {
        return Err("ghostscript missing: brew install ghostscript".into());
    }
    let limit = (target_kb * 1000.0) as usize;
    let out = output.unwrap_or_else(|| {
        input.with_file_name(format!(
            "compressed_{}",
            input.file_name().unwrap().to_string_lossy()
        ))
    });
    if out.canonicalize().ok() == input.canonicalize().ok() {
        return Err("output must differ from input".into());
    }

    let (dpi, q, bytes) = compress(&input, limit)?;
    std::fs::write(&out, &bytes).map_err(|e| format!("write {}: {e}", out.display()))?;
    let size = bytes.len();
    let in_size = std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
    let ok = size <= limit;
    println!(
        "{}\n{} KB -> {} KB  [{dpi} dpi, q{q}] {}",
        out.display(),
        in_size / 1000,
        size / 1000,
        if ok {
            "OK"
        } else {
            "OVER TARGET (text/vector heavy, cannot go lower safely)"
        }
    );
    Ok(ok)
}

fn parse_args() -> Res<(PathBuf, f64, Option<PathBuf>)> {
    let usage = "usage: compress-pdf input.pdf [-t KB (default 250)] [-o output.pdf]";
    let mut args = std::env::args().skip(1);
    let (mut parts, mut target, mut output) = (Vec::new(), 250.0, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-t" | "--target-kb" => {
                target = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|v: &f64| *v > 0.0)
                    .ok_or(usage)?
            }
            "-o" | "--output" => output = Some(PathBuf::from(args.next().ok_or(usage)?)),
            "-h" | "--help" => {
                println!("{usage}");
                std::process::exit(0);
            }
            // unquoted path with spaces arrives split by the shell: rejoin
            _ if !a.starts_with('-') => parts.push(a),
            _ => return Err(usage.into()),
        }
    }
    if parts.is_empty() {
        return Err(usage.into());
    }
    Ok((PathBuf::from(parts.join(" ")), target, output))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompressOptions {
    pub target_bytes: usize,
    pub max_dpi: Option<u32>,
    pub jpeg_quality: Option<u8>,
    pub force_recompress: bool,
}

/// Compress `input` toward `limit` bytes. Returns (dpi, quality, pdf bytes); bytes may exceed limit.
pub fn compress(input: &Path, limit: usize) -> Res<(u32, u8, Vec<u8>)> {
    compress_with_options(
        input,
        CompressOptions {
            target_bytes: limit,
            ..Default::default()
        },
    )
}

/// Compress `input` with customizable byte budget, max image DPI, and JPEG quality cap.
pub fn compress_with_options(input: &Path, opts: CompressOptions) -> Res<(u32, u8, Vec<u8>)> {
    compress_with_options_progress(input, opts, |_, _| {})
}

/// Compress `input` with customizable options while reporting monotonic `(percent, stage)` milestones.
pub fn compress_with_options_progress<F: FnMut(u8, &str)>(
    input: &Path,
    opts: CompressOptions,
    mut on_progress: F,
) -> Res<(u32, u8, Vec<u8>)> {
    if let Some(dpi) = opts.max_dpi
        && !(36..=300).contains(&dpi)
    {
        return Err("dpi must be between 36 and 300".into());
    }
    if let Some(q) = opts.jpeg_quality
        && !(15..=95).contains(&q)
    {
        return Err("quality must be between 15 and 95".into());
    }
    on_progress(24, "Inspecting PDF structure");
    let original = std::fs::read(input).map_err(|e| e.to_string())?;
    let custom_params =
        opts.max_dpi.is_some() || opts.jpeg_quality.is_some() || opts.force_recompress;
    if !custom_params && original.len() <= opts.target_bytes {
        let doc = Document::load_mem(&original).map_err(|e| format!("Invalid PDF: {e}"))?;
        if doc.get_pages().is_empty() {
            return Err("PDF has no pages".into());
        }
        on_progress(95, "Already within target size");
        return Ok((0, 0, original)); // Already fits: preserve signatures and image fidelity byte-for-byte.
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("compress-pdf-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = search_with_options(input, opts, &tmp, &mut on_progress);
    let _ = std::fs::remove_dir_all(&tmp);
    let (dpi, q, bytes) = result?;

    // sanity: output loads with same page count
    on_progress(91, "Verifying output pages");
    let pages = Document::load_mem(&bytes)
        .map_err(|e| format!("output unreadable: {e}"))?
        .get_pages()
        .len();
    if pages == 0 {
        return Err("no pages found: not a valid PDF?".into());
    }
    let orig = Document::load(input).map_err(|e| format!("not a readable PDF: {e}"))?;
    if orig.get_pages().len() != pages {
        return Err("page count changed".into());
    }
    if !custom_params && bytes.len() > original.len() {
        return Ok((0, 0, original));
    }
    Ok((dpi, q, bytes))
}

/// Returns (dpi, quality, pdf bytes).
fn search_with_options<F: FnMut(u8, &str)>(
    input: &Path,
    opts: CompressOptions,
    tmp: &Path,
    on_progress: &mut F,
) -> Res<(u32, u8, Vec<u8>)> {
    let base_dpi = opts.max_dpi.unwrap_or(BASE_DPI);
    let q_hi = opts.jpeg_quality.unwrap_or(MAX_Q).clamp(FLOOR_Q, 95);
    let q_lo = opts
        .jpeg_quality
        .map(|q| q.min(MIN_Q))
        .unwrap_or(MIN_Q)
        .clamp(FLOOR_Q, q_hi);
    let relax_custom = opts.jpeg_quality.is_some_and(|q| q < MIN_Q);
    let limit = opts.target_bytes;

    on_progress(28, "Downsampling pages");
    let gs_out = tmp.join("gs.pdf");
    gs_downsample(input, &gs_out, base_dpi)?;
    on_progress(44, "Optimizing fonts & images");
    let mut doc = Document::load(&gs_out).map_err(|e| format!("load gs output: {e}"))?;
    slim_fonts(&mut doc);
    let mut base = collect_images(&doc);
    let placed = image_dpi(&doc);
    for i in &mut base {
        // unplaced/unknown: assume gs already capped it at base_dpi
        i.dpi = placed.get(&i.id).copied().unwrap_or(base_dpi as f64);
    }
    let mut best_attempt: Option<(u32, u8, Vec<u8>)> = None;
    let mut track = |dpi: u32, q: u8, bytes: Vec<u8>| {
        if best_attempt
            .as_ref()
            .is_none_or(|(_, _, prev)| bytes.len() < prev.len())
        {
            best_attempt = Some((dpi, q, bytes));
        }
    };

    let mut dpis: Vec<u32> = DPIS.iter().copied().filter(|&d| d <= base_dpi).collect();
    if !dpis.contains(&base_dpi) {
        dpis.insert(0, base_dpi);
    }

    // Exact custom (max_dpi, jpeg_quality) attempt first when caller specifies both.
    if opts.max_dpi.is_some() && opts.jpeg_quality.is_some() {
        on_progress(52, &format!("Encoding at {base_dpi} DPI (q{q_hi})"));
        let exact_imgs = scaled(&base, base_dpi, relax_custom);
        let bytes = build(&doc, &exact_imgs, q_hi, relax_custom)?;
        if bytes.len() <= limit {
            return Ok((base_dpi, q_hi, bytes));
        }
        track(base_dpi, q_hi, bytes);
    }

    let mut imgs = Vec::new();
    let total_passes = dpis.len().max(1);
    for (idx, &dpi) in dpis.iter().enumerate() {
        let pct = (48 + (((idx + 1) * 34) / total_passes)).min(84) as u8;
        on_progress(
            pct,
            &format!("Optimizing pass {}/{} ({dpi} DPI)", idx + 1, total_passes),
        );
        imgs = scaled(&base, dpi, relax_custom);
        let fit = best_q(&doc, &imgs, limit, q_lo, q_hi, relax_custom)?;
        if let Some((q, bytes)) = fit {
            return Ok((dpi, q, bytes));
        }
        if let Ok(sample) = build(&doc, &imgs, q_lo, relax_custom) {
            track(dpi, q_lo, sample);
        }
    }
    // Second pass: drop quality floor on photos while preserving graphics.
    let dpi = *dpis.last().unwrap_or(&base_dpi);
    if q_lo > FLOOR_Q {
        on_progress(86, "Fine-tuning photo quality");
        if let Some((q, b)) = best_q(&doc, &imgs, limit, FLOOR_Q, q_lo - 1, false)? {
            return Ok((dpi, q, b));
        }
        if let Ok(sample) = build(&doc, &imgs, FLOOR_Q, false) {
            track(dpi, FLOOR_Q, sample);
        }
    }

    // Third pass: relax graphics/SMask floor and downscale further.
    on_progress(88, "Applying compact fallback pass");
    for fallback_dpi in [72u32, 60, 48, 36].into_iter().filter(|&d| d <= base_dpi) {
        let low_imgs = scaled(&base, fallback_dpi, true);
        if let Some((q, b)) = best_q(&doc, &low_imgs, limit, FLOOR_Q, q_hi, true)? {
            return Ok((fallback_dpi, q, b));
        }
        if let Ok(sample) = build(&doc, &low_imgs, FLOOR_Q, true) {
            track(fallback_dpi, FLOOR_Q, sample);
        }
    }

    // Fourth pass: rasterize complex vector/ICC/transparency pages to compact RGB JPEG PDF pages.
    on_progress(90, "Rasterizing complex pages");
    if let Some((dpi, q, b)) = gs_raster_fallback(input, limit, tmp, base_dpi, q_hi)? {
        if b.len() <= limit {
            return Ok((dpi, q, b));
        }
        track(dpi, q, b);
    }

    best_attempt.ok_or_else(|| "compression produced no output".into())
}

/// Downscale photos (and graphics if `all`) from BASE_DPI to `dpi`. Masks never change.
fn scaled(base: &[Img], dpi: u32, all: bool) -> Vec<Img> {
    base.iter()
        .map(|i| {
            let d = if (i.is_mask || i.is_gfx) && !all {
                GFX_DPI.max(dpi)
            } else {
                dpi
            };
            let f = (d as f64 / i.dpi).min(1.0); // only shrink: never upsample
            let (w, h) = (
                ((i.w as f64 * f).round() as usize).max(1),
                ((i.h as f64 * f).round() as usize).max(1),
            );
            let pixels = if (w, h) == (i.w, i.h) {
                i.pixels.clone()
            } else {
                box_resize(&i.pixels, i.w, i.h, w, h, if i.rgb { 3 } else { 1 })
            };
            Img {
                pixels,
                w,
                h,
                resized: (w, h) != (i.w, i.h),
                ..*i
            }
        })
        .collect()
}

/// Effective DPI of each image XObject: pixel width / placed width, max over all uses.
/// ponytail: page-level Do only (gs output flattens forms); images inside forms stay unscaled.
fn image_dpi(doc: &Document) -> std::collections::HashMap<ObjectId, f64> {
    let mut out = std::collections::HashMap::new();
    for (_, page) in doc.get_pages() {
        let Ok((res, ids)) = doc.get_page_resources(page) else {
            continue;
        };
        let xobj = |n: &[u8]| -> Option<ObjectId> {
            let dicts = res
                .into_iter()
                .chain(ids.iter().filter_map(|i| doc.get_dictionary(*i).ok()));
            for r in dicts {
                let x = match r.get(b"XObject") {
                    Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
                    Ok(Object::Dictionary(d)) => Some(d),
                    _ => None,
                };
                if let Some(id) = x
                    .and_then(|x| x.get(n).ok())
                    .and_then(|o| o.as_reference().ok())
                {
                    return Some(id);
                }
            }
            None
        };
        let Ok(content) = doc.get_and_decode_page_content(page) else {
            continue;
        };
        let (mut ctm, mut stack) = ([1.0f64, 0.0, 0.0, 1.0, 0.0, 0.0], Vec::new());
        for op in content.operations {
            match op.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().unwrap_or(ctm),
                "cm" if op.operands.len() == 6 => {
                    let m: Vec<f64> = op
                        .operands
                        .iter()
                        .map(|o| o.as_float().unwrap_or(0.0) as f64)
                        .collect();
                    let c = ctm;
                    ctm = [
                        m[0] * c[0] + m[1] * c[2],
                        m[0] * c[1] + m[1] * c[3],
                        m[2] * c[0] + m[3] * c[2],
                        m[2] * c[1] + m[3] * c[3],
                        m[4] * c[0] + m[5] * c[2] + c[4],
                        m[4] * c[1] + m[5] * c[3] + c[5],
                    ];
                }
                "Do" => {
                    let Some(id) = op
                        .operands
                        .first()
                        .and_then(|o| o.as_name().ok())
                        .and_then(xobj)
                    else {
                        continue;
                    };
                    let Ok(w) = doc
                        .get_object(id)
                        .and_then(|o| o.as_stream())
                        .map(|s| int(&s.dict, b"Width").unwrap_or(0))
                    else {
                        continue;
                    };
                    let inches = ctm[0].hypot(ctm[1]) / 72.0;
                    if w > 0 && inches > 0.01 {
                        let d = w as f64 / inches;
                        let e = out.entry(id).or_insert(0.0f64);
                        *e = e.max(d);
                    }
                }
                _ => {}
            }
        }
    }
    // SMasks are never drawn directly: inherit their parent's placement
    let parents: Vec<(ObjectId, f64)> = out.iter().map(|(k, v)| (*k, *v)).collect();
    for (id, d) in parents {
        let Ok(s) = doc.get_object(id).and_then(|o| o.as_stream()) else {
            continue;
        };
        let Ok(m) = s.dict.get(b"SMask").and_then(|o| o.as_reference()) else {
            continue;
        };
        let Ok(ms) = doc.get_object(m).and_then(|o| o.as_stream()) else {
            continue;
        };
        let (pw, mw) = (
            int(&s.dict, b"Width").unwrap_or(0),
            int(&ms.dict, b"Width").unwrap_or(0),
        );
        if pw > 0 && mw > 0 {
            out.insert(m, d * mw as f64 / pw as f64);
        }
    }
    out
}

/// Area-average downscale (same quality class as gs /Bicubic for shrinking).
fn box_resize(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize, ch: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * ch];
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1));
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            for c in 0..ch {
                let mut s = 0u32;
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        s += src[(yy * sw + xx) * ch + c] as u32;
                    }
                }
                out[(y * dw + x) * ch + c] = ((s + n / 2) / n) as u8;
            }
        }
    }
    out
}

fn best_q(
    doc: &Document,
    imgs: &[Img],
    limit: usize,
    mut lo: u8,
    mut hi: u8,
    relax_gfx: bool,
) -> Res<Option<(u8, Vec<u8>)>> {
    let mut found = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let bytes = build(doc, imgs, mid, relax_gfx)?;
        if bytes.len() <= limit {
            found = Some((mid, bytes));
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    Ok(found)
}

fn gs_downsample(src: &Path, dst: &Path, dpi: u32) -> Res<()> {
    let mut cmd = Command::new("gs");
    cmd.args([
        "-q",
        "-sDEVICE=pdfwrite",
        "-dCompatibilityLevel=1.6",
        "-dNOPAUSE",
        "-dBATCH",
        "-dSAFER",
        "-dColorConversionStrategy=/RGB",
        "-dProcessColorModel=/DeviceRGB",
    ])
    .args([
        "-dPassThroughJPEGImages=false",
        "-dDetectDuplicateImages=true",
    ])
    .args(["-dCompressFonts=true", "-dSubsetFonts=true"]);
    for k in ["Color", "Gray"] {
        cmd.arg(format!("-dDownsample{k}Images=true"))
            .arg(format!("-d{k}ImageResolution={dpi}"))
            .arg(format!("-d{k}ImageDownsampleThreshold=1.2"))
            .arg(format!("-d{k}ImageDownsampleType=/Bicubic"))
            .arg(format!("-dAutoFilter{k}Images=false"))
            .arg(format!("-d{k}ImageFilter=/FlateEncode"));
    }
    // `--` style guard: input path is passed after all options, never parsed as a switch
    let out = cmd
        .arg(format!("-o{}", dst.display()))
        .arg("-f")
        .arg(src)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ghostscript failed ({}): not a valid PDF?",
            out.status
        ))
    }
}

fn gs_raster_fallback(
    input: &Path,
    limit: usize,
    tmp: &Path,
    max_dpi: u32,
    max_q: u8,
) -> Res<Option<(u32, u8, Vec<u8>)>> {
    let mut smallest: Option<(u32, u8, Vec<u8>)> = None;
    for (idx, &(raw_dpi, raw_q)) in [
        (120u32, 65u8),
        (96, 55),
        (80, 45),
        (72, 38),
        (72, 26),
        (60, 20),
        (48, 16),
    ]
    .iter()
    .enumerate()
    {
        let dpi = raw_dpi.min(max_dpi);
        let q = raw_q.min(max_q);
        let rdir = tmp.join(format!("raster-{idx}"));
        std::fs::create_dir_all(&rdir).map_err(|e| e.to_string())?;
        let pattern = rdir.join("page-%04d.jpg");
        let status = Command::new("gs")
            .args([
                "-q",
                "-dNOPAUSE",
                "-dBATCH",
                "-dSAFER",
                "-sDEVICE=jpeg",
                &format!("-dJPEGQ={q}"),
                &format!("-r{dpi}"),
                &format!("-sOutputFile={}", pattern.display()),
                "-f",
            ])
            .arg(input)
            .output()
            .map_err(|e| e.to_string())?;
        if !status.status.success() {
            continue;
        }
        let mut page_files: Vec<PathBuf> = std::fs::read_dir(&rdir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jpg"))
            .collect();
        page_files.sort();
        if page_files.is_empty() {
            continue;
        }
        let pdf_bytes = wrap_jpegs_as_pdf(&page_files, dpi)?;
        if pdf_bytes.len() <= limit {
            return Ok(Some((dpi, q, pdf_bytes)));
        }
        if smallest
            .as_ref()
            .is_none_or(|(_, _, prev)| pdf_bytes.len() < prev.len())
        {
            smallest = Some((dpi, q, pdf_bytes));
        }
    }
    Ok(smallest)
}

fn wrap_jpegs_as_pdf(jpegs: &[PathBuf], dpi: u32) -> Res<Vec<u8>> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};

    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    pdf.catalog(catalog_id).pages(pages_id);

    let mut page_ids = Vec::with_capacity(jpegs.len());
    let mut next_id = 3i32;
    for path in jpegs {
        let page_id = Ref::new(next_id);
        let content_id = Ref::new(next_id + 1);
        let img_id = Ref::new(next_id + 2);
        next_id += 3;
        page_ids.push(page_id);

        let raw = std::fs::read(path).map_err(|e| e.to_string())?;
        let (w_px, h_px) = image::image_dimensions(path).map_err(|e| e.to_string())?;
        let page_w = (w_px as f32) * 72.0 / (dpi as f32);
        let page_h = (h_px as f32) * 72.0 / (dpi as f32);

        let mut img = pdf.image_xobject(img_id, &raw);
        img.filter(pdf_writer::Filter::DctDecode);
        img.width(w_px as i32);
        img.height(h_px as i32);
        img.color_space().device_rgb();
        img.bits_per_component(8);
        img.finish();

        let mut content = Content::new();
        content.save_state();
        content.transform([page_w, 0.0, 0.0, page_h, 0.0, 0.0]);
        content.x_object(Name(b"Im1"));
        content.restore_state();
        pdf.stream(content_id, &content.finish());

        let mut page = pdf.page(page_id);
        page.parent(pages_id);
        page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
        page.contents(content_id);
        page.resources().x_objects().pair(Name(b"Im1"), img_id);
        page.finish();
    }
    pdf.pages(pages_id).kids(page_ids).count(jpegs.len() as i32);
    Ok(pdf.finish())
}

// ---------- images ----------

struct Img {
    id: ObjectId,
    w: usize,
    h: usize,
    rgb: bool,
    is_mask: bool,
    is_gfx: bool,
    resized: bool,
    dpi: f64, // effective resolution where placed (max over placements)
    pixels: Vec<u8>,
    raw_len: usize,
}

fn name<'a>(d: &'a lopdf::Dictionary, k: &[u8]) -> Option<&'a [u8]> {
    d.get(k).ok().and_then(|o| o.as_name().ok())
}

fn int(d: &lopdf::Dictionary, k: &[u8]) -> Option<i64> {
    d.get(k).ok().and_then(|o| o.as_i64().ok())
}

/// Decode every 8-bit Gray/RGB Flate image once; quality sweeps reuse the pixels.
fn collect_images(doc: &Document) -> Vec<Img> {
    let masks: Vec<ObjectId> = doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter_map(|s| {
            s.dict
                .get(b"SMask")
                .ok()
                .and_then(|r| r.as_reference().ok())
        })
        .collect();
    let mut out = Vec::new();
    for (&id, obj) in &doc.objects {
        let Ok(s) = obj.as_stream() else { continue };
        let d = &s.dict;
        if name(d, b"Subtype") != Some(b"Image")
            || int(d, b"BitsPerComponent") != Some(8)
            || d.has(b"Decode")
            || (d.has(b"Matte") && !masks.contains(&id)) // Matte sits on SMasks: JPEG is fine
            || d.has(b"ImageMask")
        {
            continue;
        }
        let ch = match name(d, b"ColorSpace") {
            Some(b"DeviceRGB") => 3,
            Some(b"DeviceGray") => 1,
            _ => continue, // Indexed/ICC/CMYK: leave untouched
        };
        // only re-encode Flate (or unfiltered) data; never touch existing DCT/JPX/CCITT
        let flate_only = match d.get(b"Filter") {
            Err(_) => true,
            Ok(Object::Name(n)) => n == b"FlateDecode",
            Ok(Object::Array(a)) => a.iter().all(|f| f.as_name().ok() == Some(b"FlateDecode")),
            _ => false,
        };
        let (Some(w), Some(h)) = (int(d, b"Width"), int(d, b"Height")) else {
            continue;
        };
        if !flate_only
            || w <= 0
            || h <= 0
            || w > 65535
            || h > 65535
            || (w * h) as usize > MAX_PIXELS
        {
            continue;
        }
        let need = (w * h) as usize * ch;
        // headroom: PNG predictor adds one filter byte per row before un-predicting
        let Ok(mut pixels) = s.get_plain_content_with_limit(need + h as usize + 1024) else {
            continue;
        };
        if pixels.len() < need {
            continue;
        }
        pixels.truncate(need);
        out.push(Img {
            id,
            w: w as usize,
            h: h as usize,
            rgb: ch == 3,
            is_mask: masks.contains(&id),
            // transparent or small (logos, stamps, signatures): keep sharp
            is_gfx: d.has(b"SMask") || ((w * h) as usize) < GFX_PIXELS,
            resized: false,
            dpi: 0.0,
            pixels,
            raw_len: s.content.len(),
        });
    }
    out
}

fn jpeg(img: &Img, q: u8) -> Res<Vec<u8>> {
    // mozjpeg reports errors by unwinding; contain it
    std::panic::catch_unwind(|| -> std::io::Result<Vec<u8>> {
        let cs = if img.rgb {
            ColorSpace::JCS_RGB
        } else {
            ColorSpace::JCS_GRAYSCALE
        };
        let mut c = Compress::new(cs);
        c.set_size(img.w, img.h);
        // MS-SSIM tables (cjpeg -quant-table 2): measured better than default at equal size
        c.set_luma_qtable(&mozjpeg::qtable::MSSSIM_Luma.scaled(q as f32, q as f32));
        if img.rgb {
            c.set_chroma_qtable(&mozjpeg::qtable::MSSSIM_Chroma.scaled(q as f32, q as f32));
        }
        c.set_progressive_mode();
        c.set_optimize_coding(true);
        c.set_optimize_scans(true);
        let mut c = c.start_compress(Vec::new())?;
        c.write_scanlines(&img.pixels)?;
        c.finish()
    })
    .map_err(|_| "jpeg encoder panicked".to_string())?
    .map_err(|e| e.to_string())
}

fn build(base: &Document, imgs: &[Img], q: u8, relax_gfx: bool) -> Res<Vec<u8>> {
    let mut doc = base.clone();
    for img in imgs {
        let eff_q = if relax_gfx {
            q
        } else if img.is_mask {
            MASK_Q
        } else if img.is_gfx {
            GFX_Q.max(q)
        } else {
            q
        };
        let data = jpeg(img, eff_q)?;
        if data.len() >= img.raw_len && !img.resized {
            continue;
        }
        let Ok(Object::Stream(s)) = doc.get_object_mut(img.id) else {
            continue;
        };
        s.dict.remove(b"DecodeParms");
        s.dict.set("Width", img.w as i64);
        s.dict.set("Height", img.h as i64);
        s.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        s.set_content(data);
    }
    doc.prune_objects();
    doc.compress();
    let opts = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .compression_level(9)
        .build();
    let mut out = Vec::new();
    doc.save_with_options(&mut out, opts)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

// ---------- fonts ----------

/// Strip TrueType hinting (fpgm/prep/cvt + per-glyph instructions). Glyph shapes unchanged.
fn slim_fonts(doc: &mut Document) {
    let ids: Vec<ObjectId> = doc
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| name(d, b"Type") == Some(b"FontDescriptor"))
        .filter_map(|d| d.get(b"FontFile2").ok().and_then(|r| r.as_reference().ok()))
        .collect();
    for id in ids {
        let Ok(Object::Stream(s)) = doc.get_object_mut(id) else {
            continue;
        };
        let Ok(raw) = s.get_plain_content_with_limit(64 << 20) else {
            continue;
        };
        let Some(slim) = strip_hinting(&raw) else {
            continue;
        };
        if slim.len() < raw.len() {
            s.dict.set("Length1", slim.len() as i64);
            s.set_plain_content(slim);
            let _ = s.compress();
        }
    }
}

// PDF renderers only need head/hhea/hmtx/maxp/loca/glyf/cmap(/post header) from an embedded TrueType
const DROP: [&[u8; 4]; 16] = [
    b"fpgm", b"prep", b"cvt ", b"hdmx", b"LTSH", b"VDMX", b"DSIH", b"FFTM", b"meta", b"name",
    b"GSUB", b"GPOS", b"GDEF", b"JSTF", b"PCLT", b"kern",
];

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Returns None on anything unexpected, leaving the font untouched.
fn strip_hinting(font: &[u8]) -> Option<Vec<u8>> {
    let n = u16_at(font, 4)? as usize;
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(n);
    for i in 0..n {
        let r = 12 + i * 16;
        let tag: [u8; 4] = font.get(r..r + 4)?.try_into().ok()?;
        let (off, len) = (
            u32_at(font, r + 8)? as usize,
            u32_at(font, r + 12)? as usize,
        );
        if !DROP.contains(&&tag) {
            let mut data = font.get(off..off.checked_add(len)?)?.to_vec();
            if &tag == b"post" && data.len() >= 32 {
                data.truncate(32); // format 3: keep metrics header, drop glyph names
                data[0..4].copy_from_slice(&[0, 3, 0, 0]);
            }
            tables.push((tag, data));
        }
    }
    let find = |t: &[u8; 4]| tables.iter().position(|(k, _)| k == t);
    if let (Some(gi), Some(li), Some(hi), Some(mi)) =
        (find(b"glyf"), find(b"loca"), find(b"head"), find(b"maxp"))
    {
        let long = u16_at(&tables[hi].1, 50)? == 1;
        let glyphs = u16_at(&tables[mi].1, 4)? as usize;
        let (glyf, loca) = (&tables[gi].1, &tables[li].1);
        let at = |i: usize| -> Option<usize> {
            if long {
                u32_at(loca, i * 4).map(|v| v as usize)
            } else {
                u16_at(loca, i * 2).map(|v| v as usize * 2)
            }
        };
        let mut new_glyf = Vec::with_capacity(glyf.len());
        let mut offsets = Vec::with_capacity(glyphs + 1);
        for g in 0..glyphs {
            offsets.push(new_glyf.len());
            let (a, b) = (at(g)?, at(g + 1)?);
            if b > a {
                new_glyf.extend(strip_glyph(glyf.get(a..b)?)?);
                if new_glyf.len() % 2 == 1 {
                    new_glyf.push(0);
                }
            }
        }
        offsets.push(new_glyf.len());
        let short = !long && new_glyf.len() / 2 <= u16::MAX as usize;
        let mut new_loca = Vec::new();
        for o in offsets {
            if short {
                new_loca.extend(((o / 2) as u16).to_be_bytes());
            } else {
                new_loca.extend((o as u32).to_be_bytes());
            }
        }
        tables[gi].1 = new_glyf;
        tables[li].1 = new_loca;
        tables[hi].1[50..52].copy_from_slice(&(if short { 0u16 } else { 1 }).to_be_bytes());
        if tables[mi].1.len() >= 28 {
            tables[mi].1[26..28].copy_from_slice(&[0, 0]); // maxSizeOfInstructions
        }
    }
    Some(assemble(&font[0..4], tables))
}

fn strip_glyph(g: &[u8]) -> Option<Vec<u8>> {
    let contours = u16_at(g, 0)? as i16;
    if contours >= 0 {
        let p = 10 + 2 * contours as usize;
        let ilen = u16_at(g, p)? as usize;
        let mut out = g.get(..p)?.to_vec();
        out.extend([0, 0]);
        out.extend(g.get(p + 2 + ilen..)?);
        return Some(out);
    }
    // composite: walk components, clear WE_HAVE_INSTRUCTIONS, drop trailing program
    let mut out = g.to_vec();
    let mut p = 10;
    loop {
        let flags = u16_at(&out, p)?;
        out[p..p + 2].copy_from_slice(&(flags & !0x100).to_be_bytes());
        p += 4 + if flags & 1 != 0 { 4 } else { 2 };
        p += if flags & 8 != 0 {
            2
        } else if flags & 0x40 != 0 {
            4
        } else if flags & 0x80 != 0 {
            8
        } else {
            0
        };
        if flags & 0x20 == 0 {
            break;
        }
    }
    out.truncate(p);
    (p <= g.len()).then_some(out)
}

fn checksum(b: &[u8]) -> u32 {
    b.chunks(4).fold(0u32, |s, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(w))
    })
}

fn assemble(sfnt: &[u8], mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|t| t.0);
    let n = tables.len() as u16;
    let es = 15 - n.max(1).leading_zeros() as u16; // floor(log2 n)
    let sr = 16u16 << es;
    let mut out = sfnt.to_vec();
    for v in [n, sr, es, n * 16 - sr] {
        out.extend(v.to_be_bytes());
    }
    let mut off = 12 + 16 * tables.len();
    let mut head_off = None;
    for (tag, data) in &mut tables {
        if tag == b"head" && data.len() >= 12 {
            data[8..12].fill(0);
            head_off = Some(off);
        }
        out.extend(*tag);
        out.extend(checksum(data).to_be_bytes());
        out.extend((off as u32).to_be_bytes());
        out.extend((data.len() as u32).to_be_bytes());
        off += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        out.extend(data);
        out.resize(out.len().div_ceil(4) * 4, 0);
    }
    if let Some(h) = head_off {
        let adj = 0xB1B0AFBAu32.wrapping_sub(checksum(&out));
        out[h + 8..h + 12].copy_from_slice(&adj.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn fitting_compression_preserves_original_bytes() {
        use lopdf::{Object, Stream, dictionary};
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Contents"=>content});
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
        );
        let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        doc.trailer.set("Root", root);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input.pdf");
        doc.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let result = super::compress(&path, bytes.len()).unwrap();
        assert_eq!(result.2, bytes);
        assert_eq!((result.0, result.1), (0, 0));
        std::fs::write(&path, b"not PDF").unwrap();
        assert!(super::compress(&path, 100).is_err());
    }
    use super::*;

    #[test]
    fn simple_glyph_loses_instructions() {
        // 1 contour, bbox, endPts=[0], ilen=3, instr, flag, x, y
        let g = [
            0, 1, 0, 0, 0, 0, 0, 10, 0, 10, 0, 0, 0, 3, 9, 9, 9, 0x37, 5, 5,
        ];
        let s = strip_glyph(&g).unwrap();
        assert_eq!(s, [0, 1, 0, 0, 0, 0, 0, 10, 0, 10, 0, 0, 0, 0, 0x37, 5, 5]);
    }

    #[test]
    fn composite_glyph_drops_program() {
        // flags = ARGS_WORDS|WE_HAVE_INSTRUCTIONS, glyph 2, args (4 bytes), then ilen + program
        let g = [
            0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x01, 0, 2, 0, 1, 0, 1, 0, 2, 7, 7,
        ];
        let s = strip_glyph(&g).unwrap();
        assert_eq!(
            s,
            [
                0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x01, 0, 2, 0, 1, 0, 1
            ]
        );
    }

    #[test]
    fn heavy_smask_pdf_compresses_within_250kb_target() {
        use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};

        let w = 900i32;
        let h = 600i32;
        let mut raw_rgb = Vec::with_capacity((w * h * 3) as usize);
        let mut raw_alpha = Vec::with_capacity((w * h) as usize);
        let mut seed = 0x1234_5678_9abc_def0u64;
        for _ in 0..(w * h) {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            raw_rgb.push((seed >> 16) as u8);
            raw_rgb.push((seed >> 24) as u8);
            raw_rgb.push((seed >> 32) as u8);
            raw_alpha.push((seed >> 40) as u8);
        }

        let mut pdf = Pdf::new();
        pdf.catalog(Ref::new(1)).pages(Ref::new(2));
        pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
        let mut smask = pdf.image_xobject(Ref::new(6), &raw_alpha);
        smask.width(w);
        smask.height(h);
        smask.color_space().device_gray();
        smask.bits_per_component(8);
        smask.finish();

        let mut img = pdf.image_xobject(Ref::new(5), &raw_rgb);
        img.width(w);
        img.height(h);
        img.color_space().device_rgb();
        img.bits_per_component(8);
        img.s_mask(Ref::new(6));
        img.finish();

        let mut content = Content::new();
        content.save_state();
        content.transform([595.0, 0.0, 0.0, 400.0, 0.0, 200.0]);
        content.x_object(Name(b"Im1"));
        content.restore_state();
        pdf.stream(Ref::new(4), &content.finish());

        let mut page = pdf.page(Ref::new(3));
        page.parent(Ref::new(2));
        page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
        page.contents(Ref::new(4));
        page.resources().x_objects().pair(Name(b"Im1"), Ref::new(5));
        page.finish();

        let bytes = pdf.finish();
        assert!(
            bytes.len() > 900_000,
            "fixture should exceed 900 KB, got {}",
            bytes.len()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("NPWP.pdf");
        std::fs::write(&path, &bytes).unwrap();

        let (_dpi, _q, out) = super::compress(&path, 250_000).unwrap();
        assert!(
            out.len() <= 250_000,
            "compressed output {} exceeded 250 KB",
            out.len()
        );
    }

    #[test]
    fn compress_with_custom_dpi_and_quality_respects_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.pdf");
        let (built, _) = crate::features::resume::run(
            "resume-builder",
            &[],
            &std::collections::HashMap::from([(
                "resume_json".into(),
                r#"{"name":"DPI Test User","summary":"Checking customizable dpi and quality bounds"}"#.into(),
            )]),
            dir.path(),
        )
        .unwrap();
        std::fs::copy(&built, &path).unwrap();

        assert!(
            super::compress_with_options(
                &path,
                super::CompressOptions {
                    target_bytes: 250_000,
                    max_dpi: Some(20),
                    jpeg_quality: None,
                    force_recompress: true,
                }
            )
            .is_err()
        );
        assert!(
            super::compress_with_options(
                &path,
                super::CompressOptions {
                    target_bytes: 250_000,
                    max_dpi: Some(96),
                    jpeg_quality: Some(99),
                    force_recompress: true,
                }
            )
            .is_err()
        );

        let (dpi, q, out) = super::compress_with_options(
            &path,
            super::CompressOptions {
                target_bytes: 500_000,
                max_dpi: Some(96),
                jpeg_quality: Some(55),
                force_recompress: true,
            },
        )
        .unwrap();
        assert!(dpi <= 96);
        assert!(q <= 55);
        assert!(out.starts_with(b"%PDF-"));
    }
}
