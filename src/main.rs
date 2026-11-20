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

/// Compress `input` toward `limit` bytes. Returns (dpi, quality, pdf bytes); bytes may exceed limit.
pub fn compress(input: &Path, limit: usize) -> Res<(u32, u8, Vec<u8>)> {
    let original = std::fs::read(input).map_err(|e| e.to_string())?;
    if original.len() <= limit {
        let doc = Document::load_mem(&original).map_err(|e| format!("Invalid PDF: {e}"))?;
        if doc.get_pages().is_empty() {
            return Err("PDF has no pages".into());
        }
        return Ok((0, 0, original)); // Already fits: preserve signatures and image fidelity byte-for-byte.
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("compress-pdf-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = search(input, limit, &tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    let (dpi, q, bytes) = result?;

    // sanity: output loads with same page count
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
    if bytes.len() > original.len() {
        return Ok((0, 0, original));
    }
    Ok((dpi, q, bytes))
}

/// Returns (dpi, quality, pdf bytes).
fn search(input: &Path, limit: usize, tmp: &Path) -> Res<(u32, u8, Vec<u8>)> {
    let gs_out = tmp.join("gs.pdf");
    gs_downsample(input, &gs_out, BASE_DPI)?;
    let mut doc = Document::load(&gs_out).map_err(|e| format!("load gs output: {e}"))?;
    slim_fonts(&mut doc);
    let mut base = collect_images(&doc);
    let placed = image_dpi(&doc);
    for i in &mut base {
        // unplaced/unknown: assume gs already capped it at BASE_DPI
        i.dpi = placed.get(&i.id).copied().unwrap_or(BASE_DPI as f64);
    }
    let mut imgs = Vec::new();
    for dpi in DPIS {
        imgs = scaled(&base, dpi, false);
        let fit = best_q(&doc, &imgs, limit, MIN_Q, MAX_Q)?;
        match &fit {
            Some((q, _)) => println!("  {dpi} dpi: q{q}"),
            None => println!("  {dpi} dpi: no fit at q>={MIN_Q}"),
        }
        if let Some((q, bytes)) = fit {
            return Ok((dpi, q, bytes));
        }
    }
    // best effort: lowest dpi, drop the quality floor; last resort also shrinks graphics
    let dpi = *DPIS.last().unwrap();
    if let Some((q, b)) = best_q(&doc, &imgs, limit, FLOOR_Q, MIN_Q - 1)? {
        return Ok((dpi, q, b));
    }
    let imgs = scaled(&base, dpi, true);
    match best_q(&doc, &imgs, limit, FLOOR_Q, MAX_Q)? {
        Some((q, b)) => Ok((dpi, q, b)),
        None => Ok((dpi, FLOOR_Q, build(&doc, &imgs, FLOOR_Q)?)),
    }
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
) -> Res<Option<(u8, Vec<u8>)>> {
    let mut found = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let bytes = build(doc, imgs, mid)?;
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

fn build(base: &Document, imgs: &[Img], q: u8) -> Res<Vec<u8>> {
    let mut doc = base.clone();
    for img in imgs {
        let gfx_q = if img.is_gfx { GFX_Q.max(q) } else { q };
        let data = jpeg(img, if img.is_mask { MASK_Q } else { gfx_q })?;
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
}
