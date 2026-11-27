//! Bidirectional Office/OpenXML and offline document converters (DOCX, PPTX, XLSX, PDF, HTML, Markdown).
use crate::features::{
    conversions, pdf,
    resume::{FontKind, PdfLayout, wrap_text},
};
use lopdf::Document;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

type Res<T> = Result<T, String>;

const MAX_DOC_PAGES: usize = 200;
const MAX_ZIP_XML_BYTES: usize = 10 * 1024 * 1024;
const SLIDE_W_EMU: u64 = 12_192_000; // 16:9 widescreen (13.333 in)
const SLIDE_H_EMU: u64 = 6_858_000; // 7.5 in

const MIME_DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const MIME_PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const MIME_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

pub fn run(
    tool: &str,
    files: &[PathBuf],
    options: &HashMap<String, String>,
    dir: &Path,
) -> Res<(PathBuf, String)> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let input = files.first().ok_or("Upload a document first")?;
    if files.len() != 1 {
        return Err("This tool accepts exactly one document".into());
    }
    let input = input.canonicalize().map_err(|e| format!("Input: {e}"))?;
    let dir = dir.canonicalize().map_err(|e| e.to_string())?;
    match tool {
        "pdf-to-word" => pdf_to_word(&input, options, &dir).map(|p| (p, MIME_DOCX.into())),
        "pdf-to-powerpoint" => {
            pdf_to_powerpoint(&input, options, &dir).map(|p| (p, MIME_PPTX.into()))
        }
        "pdf-to-excel" => pdf_to_excel(&input, options, &dir).map(|p| (p, MIME_XLSX.into())),
        "word-to-pdf" | "powerpoint-to-pdf" | "excel-to-pdf" | "office-to-pdf" => {
            office_to_pdf_for(tool, &input, &dir).map(|p| (p, "application/pdf".into()))
        }
        _ => Err(format!("Unsupported office tool: {tool}")),
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code < 0x20 && !matches!(ch, '\t' | '\n' | '\r') {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn decode_xml_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';')
            && semi <= 12
        {
            let entity = &after[..semi];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                    u32::from_str_radix(&entity[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if entity.starts_with('#') => {
                    entity[1..].parse::<u32>().ok().and_then(char::from_u32)
                }
                _ => None,
            };
            if let Some(ch) = decoded {
                out.push(ch);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

pub(crate) fn extract_pdf_pages_text(input: &Path, dir: &Path) -> Res<Vec<String>> {
    let doc = Document::load(input).map_err(|e| format!("Invalid PDF: {e}"))?;
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err("PDF has no pages".into());
    }
    if pages.len() > MAX_DOC_PAGES {
        return Err(format!("Document exceeds {MAX_DOC_PAGES} page limit"));
    }

    let txt_out = dir.join("extracted-pages.txt");
    let mut page_texts = if conversions::command(
        "pdftotext",
        &[
            "-layout".into(),
            "-enc".into(),
            "UTF-8".into(),
            input.display().to_string(),
            txt_out.display().to_string(),
        ],
        dir,
    )
    .is_ok()
        && let Ok(raw) = fs::read_to_string(&txt_out)
    {
        let _ = fs::remove_file(&txt_out);
        let mut parts: Vec<String> = raw.split('\x0C').map(str::to_owned).collect();
        while parts.len() > pages.len() && parts.last().is_some_and(|s| s.trim().is_empty()) {
            parts.pop();
        }
        parts
    } else {
        Vec::new()
    };

    if page_texts.is_empty() {
        page_texts = pages
            .keys()
            .map(|&n| doc.extract_text(&[n]).unwrap_or_default())
            .collect();
    }
    if page_texts.is_empty() {
        page_texts.push(String::new());
    }
    Ok(page_texts)
}

#[derive(Clone)]
struct PdfFontSpec {
    size: f32,
    family: String,
    bold: bool,
    italic: bool,
    color: String,
}

impl Default for PdfFontSpec {
    fn default() -> Self {
        Self {
            size: 16.0,
            family: "Calibri".into(),
            bold: false,
            italic: false,
            color: "1E2430".into(),
        }
    }
}

#[derive(Clone)]
struct RawRun {
    text: String,
    bold: bool,
    italic: bool,
    size_pt: f32,
    family: String,
    color: String,
}

#[derive(Clone)]
struct RawTextItem {
    top: f32,
    left: f32,
    width: f32,
    height: f32,
    base_size: f32,
    runs: Vec<RawRun>,
}

struct RawImageItem {
    top: f32,
    left: f32,
    width: f32,
    height: f32,
    path: PathBuf,
}

struct XmlPage {
    number: usize,
    width: f32,
    height: f32,
    texts: Vec<RawTextItem>,
    images: Vec<RawImageItem>,
}

struct MediaAsset {
    target: String,
    rid: String,
    ext: String,
    bytes: Vec<u8>,
}

struct DocxBuilder {
    media: Vec<MediaAsset>,
    next_rid: usize,
    next_doc_pr: usize,
}

impl DocxBuilder {
    fn new() -> Self {
        Self {
            media: Vec::new(),
            next_rid: 2,
            next_doc_pr: 1,
        }
    }

    fn add_media(&mut self, ext: &str, bytes: Vec<u8>) -> String {
        let rid = format!("rId{}", self.next_rid);
        self.next_rid += 1;
        let clean_ext = match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => "jpg",
            "gif" => "gif",
            _ => "png",
        };
        let target = format!("media/img{}.{clean_ext}", self.media.len() + 1);
        self.media.push(MediaAsset {
            target,
            rid: rid.clone(),
            ext: clean_ext.into(),
            bytes,
        });
        rid
    }

    fn next_pr_id(&mut self) -> usize {
        let id = self.next_doc_pr;
        self.next_doc_pr += 1;
        id
    }

    fn write(self, out: &Path, body: &str) -> Res<()> {
        let file = File::create(out).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let write_entry = |z: &mut ZipWriter<File>, name: &str, data: &[u8]| -> Res<()> {
            z.start_file(name, opts).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())
        };

        let mut ext_defaults = String::from(
            r#"  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>"#,
        );
        let mut seen_exts = Vec::new();
        for m in &self.media {
            if !seen_exts.contains(&m.ext) {
                let ct = match m.ext.as_str() {
                    "jpg" => "image/jpeg",
                    "gif" => "image/gif",
                    _ => "image/png",
                };
                ext_defaults.push_str(&format!(
                    "\n  <Default Extension=\"{}\" ContentType=\"{ct}\"/>",
                    m.ext
                ));
                seen_exts.push(m.ext.clone());
            }
        }
        let content_types = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
{ext_defaults}
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#
        );
        write_entry(&mut zip, "[Content_Types].xml", content_types.as_bytes())?;

        write_entry(
            &mut zip,
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )?;

        let mut rels = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
        );
        for m in &self.media {
            rels.push_str(&format!(
                "\n  <Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{}\"/>",
                m.rid, m.target
            ));
        }
        rels.push_str("\n</Relationships>");
        write_entry(&mut zip, "word/_rels/document.xml.rels", rels.as_bytes())?;

        write_entry(
            &mut zip,
            "word/styles.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:basedOn w:val="Normal"/>
    <w:rPr><w:b/><w:sz w:val="26"/><w:szCs w:val="26"/></w:rPr>
  </w:style>
</w:styles>"#,
        )?;

        for m in &self.media {
            write_entry(&mut zip, &format!("word/{}", m.target), &m.bytes)?;
        }

        let doc_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>{body}</w:body>
</w:document>"#
        );
        write_entry(&mut zip, "word/document.xml", doc_xml.as_bytes())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn xml_attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let mut offset = 0usize;
    while let Some(pos) = attrs[offset..].find(&needle) {
        let abs = offset + pos;
        let boundary_ok = abs == 0
            || attrs.as_bytes()[abs - 1].is_ascii_whitespace()
            || attrs.as_bytes()[abs - 1] == b'<';
        if boundary_ok {
            let val_start = abs + needle.len();
            if let Some(end) = attrs[val_start..].find('"') {
                return Some(decode_xml_entities(&attrs[val_start..val_start + end]));
            }
        }
        offset = abs + needle.len();
    }
    None
}

fn strip_subset_and_style(raw: &str) -> (String, bool, bool) {
    let stripped = match raw.split_once('+') {
        Some((prefix, rest))
            if prefix.len() == 6 && prefix.bytes().all(|b| b.is_ascii_uppercase()) =>
        {
            rest.trim()
        }
        _ => raw.trim(),
    };
    let lower = stripped.to_ascii_lowercase();
    let bold = lower.contains("bold")
        || lower.contains("black")
        || lower.contains("heavy")
        || lower.contains("semibold")
        || lower.contains("demi");
    let italic = lower.contains("italic") || lower.contains("oblique") || lower.contains("slant");
    let base = stripped.split([',', '-']).next().unwrap_or(stripped).trim();
    let mapped = match base.to_ascii_lowercase().as_str() {
        "" => "Calibri".to_owned(),
        "helvetica" | "helveticaneue" => "Arial".to_owned(),
        "times" | "timesroman" | "timesnewromanpsmt" => "Times New Roman".to_owned(),
        "courier" | "courierstd" => "Courier New".to_owned(),
        _ => base.to_owned(),
    };
    (mapped, bold, italic)
}

fn parse_hex_color(raw: Option<String>) -> String {
    let s = raw.unwrap_or_default();
    let hex = s.trim().trim_start_matches('#');
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex.to_ascii_uppercase()
    } else {
        "1E2430".into()
    }
}

fn is_near_white_hex(hex: &str) -> bool {
    if hex.len() != 6 {
        return false;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    r >= 235 && g >= 235 && b >= 235
}

fn parse_text_inline_runs(inner: &str, font: &PdfFontSpec) -> Vec<RawRun> {
    let mut runs: Vec<RawRun> = Vec::new();
    let mut bold_depth = usize::from(font.bold);
    let mut italic_depth = usize::from(font.italic);
    let mut rest = inner;

    let push_slice = |runs: &mut Vec<RawRun>, chunk: &str, bold: bool, italic: bool| {
        if chunk.is_empty() {
            return;
        }
        let decoded = decode_xml_entities(chunk);
        if decoded.is_empty() {
            return;
        }
        if let Some(last) = runs.last_mut()
            && last.bold == bold
            && last.italic == italic
        {
            last.text.push_str(&decoded);
        } else {
            runs.push(RawRun {
                text: decoded,
                bold,
                italic,
                size_pt: font.size,
                family: font.family.clone(),
                color: font.color.clone(),
            });
        }
    };

    while let Some(lt) = rest.find('<') {
        push_slice(&mut runs, &rest[..lt], bold_depth > 0, italic_depth > 0);
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else {
            push_slice(&mut runs, &rest[lt..], bold_depth > 0, italic_depth > 0);
            rest = "";
            break;
        };
        let tag = after[..gt].trim();
        let name = tag.split_whitespace().next().unwrap_or("");
        match name.to_ascii_lowercase().as_str() {
            "b" | "strong" => bold_depth += 1,
            "/b" | "/strong" => bold_depth = bold_depth.saturating_sub(1),
            "i" | "em" => italic_depth += 1,
            "/i" | "/em" => italic_depth = italic_depth.saturating_sub(1),
            _ => {}
        }
        rest = &after[gt + 1..];
    }
    push_slice(&mut runs, rest, bold_depth > 0, italic_depth > 0);
    runs
}

fn resolve_xml_image(base_dir: &Path, src: &str) -> Option<PathBuf> {
    let name = Path::new(src).file_name()?.to_str()?;
    let candidate = base_dir.join(name);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn parse_pdftohtml_xml(xml: &str, base_dir: &Path) -> Vec<XmlPage> {
    let mut pages = Vec::new();
    let mut fonts: HashMap<String, PdfFontSpec> = HashMap::new();
    let mut rest = xml;
    while let Some(pos) = rest.find("<page") {
        let after = &rest[pos + 5..];
        if !matches!(after.as_bytes().first().copied(), Some(b' ' | b'>')) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else {
            break;
        };
        let attrs = &after[..gt];
        let inner = &after[gt + 1..];
        let (page_body, tail) = match inner.find("</page>") {
            Some(end) => (&inner[..end], &inner[end + 7..]),
            None => (inner, ""),
        };
        rest = tail;

        let number = xml_attr(attrs, "number")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(pages.len() + 1);
        let width = xml_attr(attrs, "width")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(892.0);
        let height = xml_attr(attrs, "height")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1263.0);

        let mut cursor = page_body;
        while let Some(fpos) = cursor.find("<fontspec") {
            let fafter = &cursor[fpos + 9..];
            let Some(fgt) = fafter.find('>') else {
                break;
            };
            let fattrs = &fafter[..fgt];
            if let Some(id) = xml_attr(fattrs, "id") {
                let raw_family = xml_attr(fattrs, "family").unwrap_or_default();
                let (family, bold, italic) = strip_subset_and_style(&raw_family);
                let size = xml_attr(fattrs, "size")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(16.0);
                let color = parse_hex_color(xml_attr(fattrs, "color"));
                fonts.insert(
                    id,
                    PdfFontSpec {
                        size,
                        family,
                        bold,
                        italic,
                        color,
                    },
                );
            }
            cursor = &fafter[fgt + 1..];
        }

        let mut images = Vec::new();
        cursor = page_body;
        while let Some(ipos) = cursor.find("<image") {
            let iafter = &cursor[ipos + 6..];
            if !matches!(iafter.as_bytes().first().copied(), Some(b' ' | b'>')) {
                cursor = iafter;
                continue;
            }
            let Some(igt) = iafter.find('>') else {
                break;
            };
            let iattrs = &iafter[..igt];
            let top = xml_attr(iattrs, "top")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let left = xml_attr(iattrs, "left")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let iw = xml_attr(iattrs, "width")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let ih = xml_attr(iattrs, "height")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            if iw >= 8.0
                && ih >= 8.0
                && let Some(src) = xml_attr(iattrs, "src")
                && let Some(path) = resolve_xml_image(base_dir, &src)
            {
                images.push(RawImageItem {
                    top,
                    left,
                    width: iw,
                    height: ih,
                    path,
                });
            }
            cursor = &iafter[igt + 1..];
        }

        let mut texts = Vec::new();
        cursor = page_body;
        while let Some(tpos) = cursor.find("<text") {
            let tafter = &cursor[tpos + 5..];
            if !matches!(tafter.as_bytes().first().copied(), Some(b' ' | b'>')) {
                cursor = tafter;
                continue;
            }
            let Some(tgt) = tafter.find('>') else {
                break;
            };
            let tattrs = &tafter[..tgt];
            if tattrs.trim_end().ends_with('/') {
                cursor = &tafter[tgt + 1..];
                continue;
            }
            let inner_start = &tafter[tgt + 1..];
            let Some(tend) = inner_start.find("</text>") else {
                break;
            };
            let inner = &inner_start[..tend];
            cursor = &inner_start[tend + 7..];

            let font_id = xml_attr(tattrs, "font").unwrap_or_default();
            let font = fonts.get(&font_id).cloned().unwrap_or_default();
            let runs = parse_text_inline_runs(inner, &font);
            if runs.iter().all(|r| r.text.is_empty()) {
                continue;
            }
            let top = xml_attr(tattrs, "top")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let left = xml_attr(tattrs, "left")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            let tw = xml_attr(tattrs, "width")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(100.0);
            let th = xml_attr(tattrs, "height")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(font.size);
            texts.push(RawTextItem {
                top,
                left,
                width: tw.max(1.0),
                height: th.max(4.0),
                base_size: font.size,
                runs,
            });
        }

        pages.push(XmlPage {
            number,
            width,
            height,
            texts,
            images,
        });
    }
    pages
}

fn pt_to_twips(pt: f32) -> i64 {
    (pt * 20.0).round() as i64
}

fn pt_to_emu(pt: f32) -> u64 {
    (pt.max(0.0) * 12_700.0).round() as u64
}

fn docx_sect_pr_xml(pt_w: f32, pt_h: f32, margin_tw: i64) -> String {
    let w_tw = pt_to_twips(pt_w).max(1440);
    let h_tw = pt_to_twips(pt_h).max(1440);
    let orient = if pt_w > pt_h {
        " w:orient=\"landscape\""
    } else {
        ""
    };
    format!(
        "<w:sectPr><w:pgSz w:w=\"{w_tw}\" w:h=\"{h_tw}\"{orient}/><w:pgMar w:top=\"{margin_tw}\" w:right=\"{margin_tw}\" w:bottom=\"{margin_tw}\" w:left=\"{margin_tw}\" w:header=\"0\" w:footer=\"0\" w:gutter=\"0\"/></w:sectPr>"
    )
}

fn docx_run_xml(run: &RawRun, remap_white: bool) -> String {
    let fam = xml_escape(&run.family);
    let half_pt = ((run.size_pt * 2.0).round() as u32).clamp(10, 288);
    let b_xml = if run.bold { "<w:b/>" } else { "" };
    let i_xml = if run.italic { "<w:i/>" } else { "" };
    let color = if remap_white && is_near_white_hex(&run.color) {
        "1E2430"
    } else {
        run.color.as_str()
    };
    let text = xml_escape(&run.text);
    format!(
        "<w:r><w:rPr><w:rFonts w:ascii=\"{fam}\" w:hAnsi=\"{fam}\" w:cs=\"{fam}\"/>{b_xml}{i_xml}<w:color w:val=\"{color}\"/><w:sz w:val=\"{half_pt}\"/><w:szCs w:val=\"{half_pt}\"/></w:rPr><w:t xml:space=\"preserve\">{text}</w:t></w:r>"
    )
}

fn docx_picture_paragraph(
    builder: &mut DocxBuilder,
    bytes: Vec<u8>,
    ext: &str,
    name: &str,
    cx: u64,
    cy: u64,
    anchor_behind: Option<(i64, i64)>,
) -> String {
    let rid = builder.add_media(ext, bytes);
    let doc_pr_id = builder.next_pr_id();
    let safe_name = xml_escape(name);
    let graphic = format!(
        "<a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:nvPicPr><pic:cNvPr id=\"0\" name=\"{safe_name}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>"
    );
    let drawing = if let Some((x_emu, y_emu)) = anchor_behind {
        format!(
            "<wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"1\" locked=\"0\" layoutInCell=\"0\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>{x_emu}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"page\"><wp:posOffset>{y_emu}</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:wrapNone/><wp:docPr id=\"{doc_pr_id}\" name=\"{safe_name}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>{graphic}</wp:anchor>"
        )
    } else {
        format!(
            "<wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:docPr id=\"{doc_pr_id}\" name=\"{safe_name}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>{graphic}</wp:inline>"
        )
    };
    format!(
        "<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/></w:pPr><w:r><w:drawing>{drawing}</w:drawing></w:r></w:p>"
    )
}

#[derive(Clone)]
struct ScaledBlock {
    top_pt: f32,
    left_pt: f32,
    right_pt: f32,
    height_pt: f32,
    size_pt: f32,
    runs: Vec<RawRun>,
}

fn scale_page_blocks(page: &XmlPage, pt_w: f32, pt_h: f32) -> Vec<ScaledBlock> {
    let sx = if page.width > 0.0 {
        page.width / pt_w
    } else {
        1.5
    };
    let sy = if page.height > 0.0 {
        page.height / pt_h
    } else {
        1.5
    };
    let mut blocks: Vec<ScaledBlock> = page
        .texts
        .iter()
        .map(|item| {
            let left_pt = (item.left / sx).clamp(0.0, pt_w - 8.0);
            let width_pt = (item.width / sx).max(1.0);
            let top_pt = (item.top / sy).clamp(0.0, pt_h - 4.0);
            let height_pt = (item.height / sy).clamp(4.0, 200.0);
            let size_pt = (item.base_size / sy).clamp(5.0, 144.0);
            let runs = item
                .runs
                .iter()
                .map(|r| RawRun {
                    text: r.text.clone(),
                    bold: r.bold,
                    italic: r.italic,
                    size_pt,
                    family: r.family.clone(),
                    color: r.color.clone(),
                })
                .collect();
            ScaledBlock {
                top_pt,
                left_pt,
                right_pt: (left_pt + width_pt).min(pt_w),
                height_pt,
                size_pt,
                runs,
            }
        })
        .collect();
    blocks.sort_by(|a, b| {
        a.top_pt
            .total_cmp(&b.top_pt)
            .then_with(|| a.left_pt.total_cmp(&b.left_pt))
    });
    blocks
}

fn cluster_visual_rows(blocks: Vec<ScaledBlock>) -> Vec<Vec<ScaledBlock>> {
    let mut rows: Vec<(f32, f32, Vec<ScaledBlock>)> = Vec::new();
    for blk in blocks {
        let mid_y = blk.top_pt + blk.height_pt * 0.5;
        let tol = (blk.height_pt * 0.45).max(2.5);
        if let Some(row) = rows
            .iter_mut()
            .rev()
            .find(|(ref_y, ref_tol, _)| (mid_y - *ref_y).abs() <= ref_tol.max(tol))
        {
            row.2.push(blk);
        } else {
            rows.push((mid_y, tol, vec![blk]));
        }
    }
    rows.into_iter()
        .map(|(_, _, mut items)| {
            items.sort_by(|a, b| a.left_pt.total_cmp(&b.left_pt));
            items
        })
        .collect()
}

fn merge_row_segments(row: Vec<ScaledBlock>, max_gap_mult: f32) -> Vec<ScaledBlock> {
    let mut out: Vec<ScaledBlock> = Vec::new();
    for blk in row {
        if let Some(prev) = out.last_mut() {
            let gap = blk.left_pt - prev.right_pt;
            let ref_pt = prev.size_pt.max(blk.size_pt).max(6.0);
            if gap <= ref_pt * max_gap_mult {
                let prev_ends_space = prev
                    .runs
                    .last()
                    .and_then(|r| r.text.chars().last())
                    .is_some_and(char::is_whitespace);
                let next_starts_space = blk
                    .runs
                    .first()
                    .and_then(|r| r.text.chars().next())
                    .is_some_and(char::is_whitespace);
                if gap > ref_pt * 0.22
                    && !prev_ends_space
                    && !next_starts_space
                    && let Some(first) = blk.runs.first()
                {
                    let mut spaced = first.clone();
                    spaced.text.insert(0, ' ');
                    prev.runs.push(spaced);
                    prev.runs.extend(blk.runs.into_iter().skip(1));
                } else {
                    prev.runs.extend(blk.runs);
                }
                prev.top_pt = prev.top_pt.min(blk.top_pt);
                prev.right_pt = prev.right_pt.max(blk.right_pt);
                prev.height_pt = prev.height_pt.max(blk.height_pt);
                prev.size_pt = prev.size_pt.max(blk.size_pt);
                continue;
            }
        }
        out.push(blk);
    }
    out
}

fn build_exact_docx(
    builder: &mut DocxBuilder,
    pages: &[XmlPage],
    page_sizes: &[(f32, f32)],
    input: &Path,
    dir: &Path,
) -> Res<String> {
    let dpi = if page_sizes.len() > 30 { 120 } else { 150 };
    let bg_pattern = dir.join("word-bg-%04d.png");
    let gs_ok = conversions::command(
        "gs",
        &[
            "-q".into(),
            "-dSAFER".into(),
            "-dBATCH".into(),
            "-dNOPAUSE".into(),
            "-dFILTERTEXT".into(),
            "-sDEVICE=png16m".into(),
            format!("-r{dpi}"),
            format!("-sOutputFile={}", bg_pattern.display()),
            "-f".into(),
            input.display().to_string(),
        ],
        dir,
    )
    .is_ok();

    let mut body = String::new();
    for (idx, &(pt_w, pt_h)) in page_sizes.iter().enumerate() {
        let page_num = idx + 1;
        let bg_path = dir.join(format!("word-bg-{page_num:04}.png"));
        let page_opt = pages
            .iter()
            .find(|p| p.number == page_num)
            .or_else(|| pages.get(idx));

        if gs_ok
            && let Ok(bg_bytes) = fs::read(&bg_path)
            && !bg_bytes.is_empty()
        {
            body.push_str(&docx_picture_paragraph(
                builder,
                bg_bytes,
                "png",
                &format!("PageBackground{page_num}"),
                pt_to_emu(pt_w),
                pt_to_emu(pt_h),
                Some((0, 0)),
            ));
        } else if let Some(page) = page_opt {
            let sx = if page.width > 0.0 {
                page.width / pt_w
            } else {
                1.5
            };
            let sy = if page.height > 0.0 {
                page.height / pt_h
            } else {
                1.5
            };
            for (img_idx, img) in page.images.iter().enumerate() {
                if let Ok(bytes) = fs::read(&img.path)
                    && !bytes.is_empty()
                {
                    let ext = img
                        .path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png");
                    body.push_str(&docx_picture_paragraph(
                        builder,
                        bytes,
                        ext,
                        &format!("Page{page_num}Img{}", img_idx + 1),
                        pt_to_emu(img.width / sx),
                        pt_to_emu(img.height / sy),
                        Some((
                            pt_to_emu(img.left / sx) as i64,
                            pt_to_emu(img.top / sy) as i64,
                        )),
                    ));
                }
            }
        }

        if let Some(page) = page_opt {
            let rows = cluster_visual_rows(scale_page_blocks(page, pt_w, pt_h));
            for row in rows {
                for seg in merge_row_segments(row, 1.45) {
                    if seg.runs.iter().all(|r| r.text.trim().is_empty()) {
                        continue;
                    }
                    let x_tw = pt_to_twips(seg.left_pt).max(0);
                    let y_tw = pt_to_twips(seg.top_pt).max(0);
                    let max_w_tw = (pt_to_twips(pt_w) - x_tw).max(480);
                    let w_tw =
                        pt_to_twips((seg.right_pt - seg.left_pt) + 18.0).clamp(480, max_w_tw);
                    body.push_str(&format!(
                        "<w:p><w:pPr><w:framePr w:w=\"{w_tw}\" w:wrap=\"none\" w:vAnchor=\"page\" w:hAnchor=\"page\" w:x=\"{x_tw}\" w:y=\"{y_tw}\"/><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>"
                    ));
                    for run in &seg.runs {
                        body.push_str(&docx_run_xml(run, false));
                    }
                    body.push_str("</w:p>");
                }
            }
        }

        let sect = docx_sect_pr_xml(pt_w, pt_h, 0);
        if idx + 1 < page_sizes.len() {
            body.push_str(&format!(
                "<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/>{sect}</w:pPr></w:p>"
            ));
        } else {
            body.push_str(&sect);
        }
    }
    Ok(body)
}

fn segment_plain_text(seg: &ScaledBlock) -> String {
    seg.runs.iter().map(|r| r.text.as_str()).collect()
}

fn emit_flow_paragraph(body: &mut String, segs: &[ScaledBlock], body_pt: f32, space_after_tw: i64) {
    let max_sz = segs.iter().map(|s| s.size_pt).fold(0.0_f32, f32::max);
    let text_preview: String = segs.iter().map(segment_plain_text).collect();
    let trimmed = text_preview.trim();
    if trimmed.is_empty() {
        return;
    }
    let is_bullet = trimmed.starts_with('•')
        || trimmed.starts_with('▪')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ");
    let style = if !is_bullet && trimmed.len() < 120 && max_sz >= body_pt * 1.45 {
        "Heading1"
    } else if !is_bullet && trimmed.len() < 120 && max_sz >= body_pt * 1.22 {
        "Heading2"
    } else {
        "Normal"
    };
    let ind = if is_bullet {
        "<w:ind w:left=\"360\" w:hanging=\"180\"/>"
    } else {
        ""
    };
    let before = if style == "Normal" { 0 } else { 140 };
    body.push_str(&format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{style}\"/>{ind}<w:spacing w:before=\"{before}\" w:after=\"{space_after_tw}\"/></w:pPr>"
    ));
    for (idx, seg) in segs.iter().enumerate() {
        if idx > 0 {
            let need_space = !segs[idx - 1]
                .runs
                .last()
                .and_then(|r| r.text.chars().last())
                .is_some_and(char::is_whitespace)
                && !seg
                    .runs
                    .first()
                    .and_then(|r| r.text.chars().next())
                    .is_some_and(char::is_whitespace);
            if need_space && let Some(first) = seg.runs.first() {
                let mut sp = first.clone();
                sp.text = " ".into();
                body.push_str(&docx_run_xml(&sp, true));
            }
        }
        for run in &seg.runs {
            body.push_str(&docx_run_xml(run, true));
        }
    }
    body.push_str("</w:p>");
}

fn emit_flow_table(body: &mut String, table_rows: &[Vec<ScaledBlock>], content_w_tw: i64) {
    let cols = table_rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let col_w = (content_w_tw / cols as i64).max(720);
    body.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D1D5DB\"/><w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D1D5DB\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D1D5DB\"/><w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D1D5DB\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"E5E7EB\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"E5E7EB\"/></w:tblBorders></w:tblPr>");
    for row in table_rows {
        body.push_str("<w:tr>");
        for c in 0..cols {
            body.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{col_w}\" w:type=\"dxa\"/></w:tcPr>"
            ));
            if let Some(cell) = row.get(c) {
                body.push_str("<w:p><w:pPr><w:spacing w:before=\"40\" w:after=\"40\"/></w:pPr>");
                for run in &cell.runs {
                    body.push_str(&docx_run_xml(run, true));
                }
                body.push_str("</w:p>");
            } else {
                body.push_str("<w:p/>");
            }
            body.push_str("</w:tc>");
        }
        body.push_str("</w:tr>");
    }
    body.push_str("</w:tbl>");
}

fn median_body_size(pages: &[XmlPage], page_sizes: &[(f32, f32)]) -> f32 {
    let mut sizes = Vec::new();
    for (idx, page) in pages.iter().enumerate() {
        let pt_h = page_sizes
            .get(page.number.saturating_sub(1))
            .or_else(|| page_sizes.get(idx))
            .map(|s| s.1)
            .unwrap_or(841.89);
        let sy = if page.height > 0.0 {
            page.height / pt_h
        } else {
            1.5
        };
        for t in &page.texts {
            sizes.push((t.base_size / sy).clamp(6.0, 72.0));
        }
    }
    if sizes.is_empty() {
        return 11.0;
    }
    sizes.sort_by(f32::total_cmp);
    sizes[sizes.len() / 2]
}

fn build_flow_docx(
    builder: &mut DocxBuilder,
    pages: &[XmlPage],
    page_sizes: &[(f32, f32)],
) -> String {
    let margin_tw = 1080_i64; // 0.75 in
    let body_pt = median_body_size(pages, page_sizes);
    let mut body = String::new();

    for (idx, &(pt_w, pt_h)) in page_sizes.iter().enumerate() {
        let page_num = idx + 1;
        let content_w_pt = (pt_w - 108.0).max(200.0);
        let content_w_tw = pt_to_twips(content_w_pt);
        if let Some(page) = pages
            .iter()
            .find(|p| p.number == page_num)
            .or_else(|| pages.get(idx))
        {
            let sx = if page.width > 0.0 {
                page.width / pt_w
            } else {
                1.5
            };
            let sy = if page.height > 0.0 {
                page.height / pt_h
            } else {
                1.5
            };
            let split_tabular_spaces = |row: Vec<ScaledBlock>| -> Vec<ScaledBlock> {
                let mut cells = Vec::new();
                for blk in row {
                    let has_gap = blk.runs.iter().any(|r| r.text.contains("   "));
                    if !has_gap {
                        cells.push(blk);
                        continue;
                    }
                    let mut cur_runs: Vec<RawRun> = Vec::new();
                    let mut col_idx = 0usize;
                    for run in &blk.runs {
                        let mut rest = run.text.as_str();
                        while let Some(pos) = rest.find("   ") {
                            let head = rest[..pos].trim_end_matches(' ');
                            if !head.is_empty() {
                                let mut part = run.clone();
                                part.text = head.to_owned();
                                cur_runs.push(part);
                            }
                            if !cur_runs.is_empty() {
                                let mut c = blk.clone();
                                c.left_pt = blk.left_pt + (col_idx as f32) * 40.0;
                                c.runs = std::mem::take(&mut cur_runs);
                                cells.push(c);
                                col_idx += 1;
                            }
                            let after = &rest[pos + 3..];
                            rest = after.trim_start_matches(' ');
                        }
                        if !rest.trim().is_empty() {
                            let mut tail = run.clone();
                            tail.text = rest.trim().to_owned();
                            cur_runs.push(tail);
                        }
                    }
                    if !cur_runs.is_empty() {
                        let mut c = blk.clone();
                        c.left_pt = blk.left_pt + (col_idx as f32) * 40.0;
                        c.runs = cur_runs;
                        cells.push(c);
                    }
                }
                cells
            };

            let rows: Vec<Vec<ScaledBlock>> =
                cluster_visual_rows(scale_page_blocks(page, pt_w, pt_h))
                    .into_iter()
                    .map(|r| split_tabular_spaces(merge_row_segments(r, 1.45)))
                    .filter(|r| !r.is_empty())
                    .collect();

            let mut sorted_imgs: Vec<&RawImageItem> = page.images.iter().collect();
            sorted_imgs.sort_by(|a, b| a.top.total_cmp(&b.top));
            let mut img_cursor = 0usize;

            let mut i = 0usize;
            let mut para_buf: Vec<ScaledBlock> = Vec::new();
            let mut para_bottom = 0.0_f32;
            let mut para_left = 0.0_f32;
            let mut para_size = body_pt;

            let flush_para = |body: &mut String, buf: &mut Vec<ScaledBlock>| {
                if !buf.is_empty() {
                    emit_flow_paragraph(body, buf, body_pt, 100);
                    buf.clear();
                }
            };

            let emit_inline_img = |body: &mut String,
                                   builder: &mut DocxBuilder,
                                   img: &RawImageItem,
                                   label: String| {
                if let Ok(bytes) = fs::read(&img.path)
                    && !bytes.is_empty()
                {
                    let mut w_pt = (img.width / sx).max(16.0);
                    let mut h_pt = (img.height / sy).max(16.0);
                    if w_pt > content_w_pt {
                        let ratio = content_w_pt / w_pt;
                        w_pt = content_w_pt;
                        h_pt *= ratio;
                    }
                    let ext = img
                        .path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png");
                    body.push_str(&docx_picture_paragraph(
                        builder,
                        bytes,
                        ext,
                        &label,
                        pt_to_emu(w_pt),
                        pt_to_emu(h_pt),
                        None,
                    ));
                }
            };

            while i < rows.len() {
                let row_top = rows[i][0].top_pt;
                while img_cursor < sorted_imgs.len()
                    && (sorted_imgs[img_cursor].top / sy) <= row_top
                {
                    flush_para(&mut body, &mut para_buf);
                    let img = sorted_imgs[img_cursor];
                    img_cursor += 1;
                    emit_inline_img(
                        &mut body,
                        builder,
                        img,
                        format!("Page{page_num}Inline{img_cursor}"),
                    );
                }

                if rows[i].len() >= 2
                    && i + 1 < rows.len()
                    && rows[i + 1].len() >= 2
                    && (rows[i + 1][0].top_pt - (rows[i][0].top_pt + rows[i][0].height_pt))
                        <= body_pt * 3.0
                {
                    flush_para(&mut body, &mut para_buf);
                    let start = i;
                    i += 2;
                    while i < rows.len()
                        && rows[i].len() >= 2
                        && (rows[i][0].top_pt - (rows[i - 1][0].top_pt + rows[i - 1][0].height_pt))
                            <= body_pt * 3.0
                    {
                        i += 1;
                    }
                    emit_flow_table(&mut body, &rows[start..i], content_w_tw);
                    continue;
                }

                let row = &rows[i];
                let first = &row[0];
                let text: String = row.iter().map(segment_plain_text).collect();
                let t = text.trim();
                let is_bullet = t.starts_with('•')
                    || t.starts_with('▪')
                    || t.starts_with("- ")
                    || t.starts_with("* ");
                let is_heading = row.iter().map(|s| s.size_pt).fold(0.0_f32, f32::max)
                    >= body_pt * 1.22
                    && t.len() < 120;
                let row_bottom = row
                    .iter()
                    .map(|s| s.top_pt + s.height_pt)
                    .fold(0.0_f32, f32::max);

                if row.len() >= 2 || is_bullet || is_heading {
                    flush_para(&mut body, &mut para_buf);
                    emit_flow_paragraph(&mut body, row, body_pt, 100);
                } else {
                    let gap_y = first.top_pt - para_bottom;
                    let same_col = (first.left_pt - para_left).abs() <= 18.0;
                    let same_sz = (first.size_pt - para_size).abs() <= 1.5;
                    if !para_buf.is_empty() && same_col && same_sz && gap_y <= para_size * 0.7 {
                        para_buf.push(first.clone());
                        para_bottom = row_bottom;
                    } else {
                        flush_para(&mut body, &mut para_buf);
                        para_left = first.left_pt;
                        para_size = first.size_pt;
                        para_bottom = row_bottom;
                        para_buf.push(first.clone());
                    }
                }
                i += 1;
            }
            flush_para(&mut body, &mut para_buf);

            while img_cursor < sorted_imgs.len() {
                let img = sorted_imgs[img_cursor];
                img_cursor += 1;
                emit_inline_img(
                    &mut body,
                    builder,
                    img,
                    format!("Page{page_num}Inline{img_cursor}"),
                );
            }
        }

        let sect = docx_sect_pr_xml(pt_w, pt_h, margin_tw);
        if idx + 1 < page_sizes.len() {
            body.push_str(&format!("<w:p><w:pPr>{sect}</w:pPr></w:p>"));
        } else {
            body.push_str(&sect);
        }
    }
    body
}

fn build_plain_fallback_docx(pages: &[String], page_sizes: &[(f32, f32)]) -> String {
    let mut body = String::new();
    for (idx, page_text) in pages.iter().enumerate() {
        let &(pt_w, pt_h) = page_sizes.get(idx).unwrap_or(&(595.28, 841.89));
        let mut any_line = false;
        for line in page_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            any_line = true;
            body.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            body.push_str(&xml_escape(trimmed));
            body.push_str("</w:t></w:r></w:p>");
        }
        if !any_line {
            body.push_str("<w:p><w:r><w:t/></w:r></w:p>");
        }
        let sect = docx_sect_pr_xml(pt_w, pt_h, 1440);
        if idx + 1 < pages.len() {
            body.push_str(&format!("<w:p><w:pPr>{sect}</w:pPr></w:p>"));
        } else {
            body.push_str(&sect);
        }
    }
    body
}

fn pdf2docx_python_candidates() -> Vec<String> {
    let mut list = Vec::new();
    let push_unique = |out: &mut Vec<String>, cand: String| {
        if !cand.is_empty() && !out.contains(&cand) {
            out.push(cand);
        }
    };
    if let Ok(env_py) = std::env::var("PDF2DOCX_PYTHON") {
        push_unique(&mut list, env_py.trim().to_owned());
    }
    let check_root = |root: &Path, out: &mut Vec<String>| {
        for rel in [
            ".venv-pdf2docx/bin/python",
            ".venv-pdf2docx/Scripts/python.exe",
        ] {
            let p = root.join(rel);
            if p.is_file() {
                push_unique(out, p.display().to_string());
            }
        }
    };
    if let Ok(cwd) = std::env::current_dir() {
        check_root(&cwd, &mut list);
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(5) {
            check_root(dir, &mut list);
        }
    }
    push_unique(&mut list, "python3".to_owned());
    list
}

fn pdf2docx_script_path() -> Option<PathBuf> {
    if let Ok(env_script) = std::env::var("PDF2DOCX_SCRIPT") {
        let p = PathBuf::from(env_script.trim());
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("scripts/pdf_to_word.py");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(5) {
            let p = dir.join("scripts/pdf_to_word.py");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn try_pdf2docx(input: &Path, out: &Path, dir: &Path, mode: &str, ocr_lang: &str) -> Res<()> {
    const INLINE_FALLBACK: &str = "import sys\nfrom pdf2docx import Converter\ncv = Converter(sys.argv[1])\ntry:\n    cv.convert(sys.argv[2], start=0, end=None)\nfinally:\n    cv.close()\n";
    let script_path = pdf2docx_script_path();
    let py_mode = if mode == "ocr" { "ocr" } else { "auto" };
    let mut last_err = String::from("PDF-to-Word conversion engine not found");
    for py in pdf2docx_python_candidates() {
        let _ = fs::remove_file(out);
        let args: Vec<String> = if let Some(ref sp) = script_path {
            vec![
                sp.display().to_string(),
                input.display().to_string(),
                out.display().to_string(),
                "--mode".into(),
                py_mode.into(),
                "--lang".into(),
                ocr_lang.into(),
            ]
        } else {
            vec![
                "-c".into(),
                INLINE_FALLBACK.into(),
                input.display().to_string(),
                out.display().to_string(),
            ]
        };
        let res = conversions::command(&py, &args, dir);
        match res {
            Ok(()) => {
                if let Ok(bytes) = fs::read(out)
                    && bytes.len() > 200
                    && bytes.starts_with(b"PK")
                {
                    return Ok(());
                }
                last_err = format!("{py} exited cleanly without valid DOCX");
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

fn validate_ocr_lang(raw: Option<&str>) -> Res<String> {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return Ok("ind+eng".into());
    }
    if trimmed.len() > 32 || !trimmed.bytes().all(|b| b.is_ascii_lowercase() || b == b'+') {
        return Err("ocr_language must be codes like ind+eng or eng".into());
    }
    Ok(trimmed.to_owned())
}

fn pdf_to_word(input: &Path, options: &HashMap<String, String>, dir: &Path) -> Res<PathBuf> {
    let mode = options
        .get("mode")
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "auto".into());
    let mode = if mode == "editable" {
        "auto".to_owned()
    } else {
        mode
    };
    if !matches!(mode.as_str(), "auto" | "ocr" | "exact" | "flow") {
        return Err("mode must be auto, ocr, exact, or flow".into());
    }
    let ocr_lang = validate_ocr_lang(options.get("ocr_language").map(String::as_str))?;

    let doc = Document::load(input).map_err(|e| format!("Invalid PDF: {e}"))?;
    let pages_map = doc.get_pages();
    if pages_map.is_empty() {
        return Err("PDF has no pages".into());
    }
    if pages_map.len() > MAX_DOC_PAGES {
        return Err(format!("Document exceeds {MAX_DOC_PAGES} page limit"));
    }

    let out = dir.join("result.docx");
    if mode == "ocr" {
        try_pdf2docx(input, &out, dir, "ocr", &ocr_lang)?;
        return Ok(out);
    }
    if mode == "auto" && try_pdf2docx(input, &out, dir, "auto", &ocr_lang).is_ok() {
        return Ok(out);
    }

    let page_sizes = pdf::page_visual_sizes_pt(&doc);
    let xml_out = dir.join("pdftohtml-out.xml");
    let xml_pages = if conversions::command(
        "pdftohtml",
        &[
            "-xml".into(),
            "-hidden".into(),
            input.display().to_string(),
            xml_out.display().to_string(),
        ],
        dir,
    )
    .is_ok()
        && let Ok(xml) = fs::read_to_string(&xml_out)
    {
        let _ = fs::remove_file(&xml_out);
        parse_pdftohtml_xml(&xml, dir)
    } else {
        Vec::new()
    };

    let has_xml_text = xml_pages.iter().any(|p| !p.texts.is_empty());
    let mut builder = DocxBuilder::new();
    let body = if has_xml_text {
        if matches!(mode.as_str(), "auto" | "exact") {
            build_exact_docx(&mut builder, &xml_pages, &page_sizes, input, dir)?
        } else {
            build_flow_docx(&mut builder, &xml_pages, &page_sizes)
        }
    } else {
        let plain_pages = extract_pdf_pages_text(input, dir)?;
        if plain_pages.iter().all(|p| p.trim().is_empty()) {
            return Err("No extractable text found in PDF. Run OCR PDF first.".into());
        }
        build_plain_fallback_docx(&plain_pages, &page_sizes)
    };

    builder.write(&out, &body)?;
    Ok(out)
}

fn pdf_to_powerpoint(input: &Path, options: &HashMap<String, String>, dir: &Path) -> Res<PathBuf> {
    let doc = Document::load(input).map_err(|e| format!("Invalid PDF: {e}"))?;
    let count = doc.get_pages().len();
    if count == 0 || count > 50 {
        return Err("PDF to PowerPoint supports 1 to 50 pages".into());
    }
    let mode = options
        .get("mode")
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "visual".into());
    if !matches!(mode.as_str(), "visual" | "editable") {
        return Err("mode must be visual or editable".into());
    }

    let mut slide_images: Vec<Vec<u8>> = Vec::new();
    if mode == "visual" {
        let mut ok = true;
        for n in 1..=count {
            let img_path = dir.join(format!("ppt-slide-{n}.jpg"));
            let res = conversions::command(
                "gs",
                &[
                    "-q".into(),
                    "-dSAFER".into(),
                    "-dBATCH".into(),
                    "-dNOPAUSE".into(),
                    "-sDEVICE=jpeg".into(),
                    "-dJPEGQ=85".into(),
                    "-r150".into(),
                    format!("-dFirstPage={n}"),
                    format!("-dLastPage={n}"),
                    format!("-o{}", img_path.display()),
                    "-f".into(),
                    input.display().to_string(),
                ],
                dir,
            );
            if res.is_ok()
                && let Ok(bytes) = fs::read(&img_path)
                && !bytes.is_empty()
            {
                let _ = fs::remove_file(&img_path);
                slide_images.push(bytes);
            } else {
                ok = false;
                break;
            }
        }
        if !ok {
            slide_images.clear();
        }
    }

    let page_texts = if slide_images.is_empty() {
        extract_pdf_pages_text(input, dir)?
    } else {
        Vec::new()
    };
    let slide_count = if !slide_images.is_empty() {
        slide_images.len()
    } else {
        page_texts.len()
    };

    let out = dir.join("result.pptx");
    let file = File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let write_bytes = |z: &mut ZipWriter<File>, name: &str, data: &[u8]| -> Res<()> {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(data).map_err(|e| e.to_string())
    };

    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#,
    );
    for i in 1..=slide_count {
        content_types.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }
    content_types.push_str("</Types>");
    write_bytes(&mut zip, "[Content_Types].xml", content_types.as_bytes())?;

    write_bytes(
        &mut zip,
        "_rels/.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#,
    )?;

    let mut pres_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    let mut sld_id_lst = String::new();
    for i in 1..=slide_count {
        let rid = i + 1;
        let sid = 255 + i;
        pres_rels.push_str(&format!(
            "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{i}.xml\"/>"
        ));
        sld_id_lst.push_str(&format!("<p:sldId id=\"{sid}\" r:id=\"rId{rid}\"/>"));
    }
    pres_rels.push_str("</Relationships>");
    write_bytes(
        &mut zip,
        "ppt/_rels/presentation.xml.rels",
        pres_rels.as_bytes(),
    )?;

    let pres_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
  <p:sldIdLst>{sld_id_lst}</p:sldIdLst>
  <p:sldSz cx="{SLIDE_W_EMU}" cy="{SLIDE_H_EMU}"/>
  <p:notesSz cx="{SLIDE_H_EMU}" cy="{SLIDE_W_EMU}"/>
</p:presentation>"#
    );
    write_bytes(&mut zip, "ppt/presentation.xml", pres_xml.as_bytes())?;

    write_bytes(
        &mut zip,
        "ppt/theme/theme1.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="520pdf Theme">
  <a:themeElements>
    <a:clrScheme name="520pdf">
      <a:dk1><a:sysClr val="windowText" lastClr="111318"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="1F2937"/></a:dk2>
      <a:lt2><a:srgbClr val="F8FAFC"/></a:lt2>
      <a:accent1><a:srgbClr val="E63228"/></a:accent1>
      <a:accent2><a:srgbClr val="2563EB"/></a:accent2>
      <a:accent3><a:srgbClr val="16A34A"/></a:accent3>
      <a:accent4><a:srgbClr val="CA8A04"/></a:accent4>
      <a:accent5><a:srgbClr val="7C3AED"/></a:accent5>
      <a:accent6><a:srgbClr val="0891B2"/></a:accent6>
      <a:hlink><a:srgbClr val="2563EB"/></a:hlink>
      <a:folHlink><a:srgbClr val="7C3AED"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="520pdf"><a:majorFont><a:latin typeface="Calibri"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme>
    <a:fmtScheme name="520pdf"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme>
  </a:themeElements>
</a:theme>"#,
    )?;

    write_bytes(
        &mut zip,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#,
    )?;

    write_bytes(
        &mut zip,
        "ppt/slideMasters/slideMaster1.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>"#,
    )?;

    write_bytes(
        &mut zip,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#,
    )?;

    write_bytes(
        &mut zip,
        "ppt/slideLayouts/slideLayout1.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1">
  <p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>
</p:sldLayout>"#,
    )?;

    for idx in 0..slide_count {
        let slide_num = idx + 1;
        if !slide_images.is_empty() {
            let img_bytes = &slide_images[idx];
            let (iw, ih) = image::load_from_memory(img_bytes)
                .map(|im| (im.width().max(1) as u64, im.height().max(1) as u64))
                .unwrap_or((16, 9));
            let (cx, cy) = if iw * SLIDE_H_EMU > ih * SLIDE_W_EMU {
                (SLIDE_W_EMU, (SLIDE_W_EMU * ih / iw).max(1))
            } else {
                ((SLIDE_H_EMU * iw / ih).max(1), SLIDE_H_EMU)
            };
            let off_x = (SLIDE_W_EMU.saturating_sub(cx)) / 2;
            let off_y = (SLIDE_H_EMU.saturating_sub(cy)) / 2;

            write_bytes(
                &mut zip,
                &format!("ppt/media/slide{slide_num}.jpg"),
                img_bytes,
            )?;
            let slide_rels = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/slide{slide_num}.jpg"/>
</Relationships>"#
            );
            write_bytes(
                &mut zip,
                &format!("ppt/slides/_rels/slide{slide_num}.xml.rels"),
                slide_rels.as_bytes(),
            )?;
            let slide_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:pic>
        <p:nvPicPr><p:cNvPr id="2" name="Page {slide_num}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>
        <p:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
        <p:spPr><a:xfrm><a:off x="{off_x}" y="{off_y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
      </p:pic>
    </p:spTree>
  </p:cSld>
</p:sld>"#
            );
            write_bytes(
                &mut zip,
                &format!("ppt/slides/slide{slide_num}.xml"),
                slide_xml.as_bytes(),
            )?;
        } else {
            let slide_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;
            write_bytes(
                &mut zip,
                &format!("ppt/slides/_rels/slide{slide_num}.xml.rels"),
                slide_rels,
            )?;
            let lines: Vec<String> = page_texts[idx]
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect();
            let title = lines
                .first()
                .cloned()
                .unwrap_or_else(|| format!("Slide {slide_num}"));
            let mut body_paras = String::new();
            for line in lines.iter().skip(1) {
                body_paras.push_str("<a:p><a:r><a:rPr lang=\"en-US\" sz=\"1500\"/><a:t>");
                body_paras.push_str(&xml_escape(line));
                body_paras.push_str("</a:t></a:r></a:p>");
            }
            if body_paras.is_empty() {
                body_paras.push_str("<a:p><a:endParaRPr lang=\"en-US\" sz=\"1500\"/></a:p>");
            }
            let slide_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="685800" y="457200"/><a:ext cx="10820400" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" sz="2400" b="1"/><a:t>{}</a:t></a:r></a:p></p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="685800" y="1524000"/><a:ext cx="10820400" cy="4800600"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        <p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>{body_paras}</p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
                xml_escape(&title)
            );
            write_bytes(
                &mut zip,
                &format!("ppt/slides/slide{slide_num}.xml"),
                slide_xml.as_bytes(),
            )?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

pub(crate) fn split_table_columns(line: &str) -> Vec<String> {
    if line.contains('\t') {
        return line
            .split('\t')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut spaces = 0usize;
    for ch in line.chars() {
        if ch == ' ' {
            spaces += 1;
        } else {
            if spaces >= 2 && !cur.trim().is_empty() {
                cells.push(cur.trim().to_owned());
                cur.clear();
            } else if spaces == 1 && !cur.is_empty() {
                cur.push(' ');
            }
            spaces = 0;
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        cells.push(cur.trim().to_owned());
    }
    cells
}

fn col_letter(mut idx: usize) -> String {
    let mut chars = Vec::new();
    loop {
        chars.push((b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    chars.into_iter().rev().collect()
}

fn pdf_to_excel(input: &Path, options: &HashMap<String, String>, dir: &Path) -> Res<PathBuf> {
    let pages = extract_pdf_pages_text(input, dir)?;
    if pages.iter().all(|p| p.trim().is_empty()) {
        return Err("No extractable text found in PDF. Run OCR PDF first.".into());
    }
    let merge = options.get("merge_sheets").is_some_and(|v| v == "true");

    let mut sheets: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    if merge {
        let mut all_rows = Vec::new();
        for page in &pages {
            for line in page.lines() {
                let cols = split_table_columns(line);
                if !cols.is_empty() {
                    all_rows.push(cols);
                }
            }
        }
        sheets.push(("Document".into(), all_rows));
    } else {
        for (i, page) in pages.iter().enumerate() {
            let mut rows = Vec::new();
            for line in page.lines() {
                let cols = split_table_columns(line);
                if !cols.is_empty() {
                    rows.push(cols);
                }
            }
            if rows.is_empty() {
                rows.push(vec![String::new()]);
            }
            sheets.push((format!("Page {}", i + 1), rows));
        }
    }

    let out = dir.join("result.xlsx");
    let file = File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let write_xml = |z: &mut ZipWriter<File>, name: &str, body: &str| -> Res<()> {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };

    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    let mut wb_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    );
    let mut wb_sheets = String::new();
    for (idx, (name, _)) in sheets.iter().enumerate() {
        let n = idx + 1;
        let rid = n + 1;
        ct.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        ));
        wb_rels.push_str(&format!(
            "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"
        ));
        wb_sheets.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{rid}\"/>",
            xml_escape(name)
        ));
    }
    ct.push_str("</Types>");
    wb_rels.push_str("</Relationships>");

    write_xml(&mut zip, "[Content_Types].xml", &ct)?;
    write_xml(
        &mut zip,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )?;
    write_xml(&mut zip, "xl/_rels/workbook.xml.rels", &wb_rels)?;
    write_xml(
        &mut zip,
        "xl/workbook.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>{wb_sheets}</sheets>
</workbook>"#
        ),
    )?;
    write_xml(
        &mut zip,
        "xl/styles.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>
  <fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>
  <borders count="1"><border/></borders>
  <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
  <cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>
</styleSheet>"#,
    )?;

    for (idx, (_, rows)) in sheets.iter().enumerate() {
        let mut sheet = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
        );
        for (r_idx, cols) in rows.iter().enumerate() {
            let row_num = r_idx + 1;
            sheet.push_str(&format!("<row r=\"{row_num}\">"));
            for (c_idx, val) in cols.iter().enumerate() {
                let cell_ref = format!("{}{row_num}", col_letter(c_idx));
                let trimmed = val.trim();
                if let Ok(num) = trimmed.parse::<f64>()
                    && num.is_finite()
                    && !trimmed.starts_with('0')
                {
                    sheet.push_str(&format!("<c r=\"{cell_ref}\"><v>{num}</v></c>"));
                } else {
                    sheet.push_str(&format!(
                        "<c r=\"{cell_ref}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        xml_escape(trimmed)
                    ));
                }
            }
            sheet.push_str("</row>");
        }
        sheet.push_str("</sheetData></worksheet>");
        write_xml(
            &mut zip,
            &format!("xl/worksheets/sheet{}.xml", idx + 1),
            &sheet,
        )?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

fn read_zip_entry_capped<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry
        .take((MAX_ZIP_XML_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.len() > MAX_ZIP_XML_BYTES {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn extract_tag_runs(segment: &str, text_tag: &str) -> String {
    let mut out = String::new();
    let open_prefix = format!("<{text_tag}");
    let close_tag = format!("</{text_tag}>");
    let mut rest = segment;
    while let Some(pos) = rest.find(&open_prefix) {
        let after = &rest[pos + open_prefix.len()..];
        let next = after.as_bytes().first().copied();
        if !matches!(next, Some(b'>' | b' ')) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else {
            break;
        };
        let content_start = &after[gt + 1..];
        if after[..gt].ends_with('/') {
            rest = content_start;
            continue;
        }
        if let Some(end) = content_start.find(&close_tag) {
            out.push_str(&decode_xml_entities(&content_start[..end]));
            rest = &content_start[end + close_tag.len()..];
        } else {
            break;
        }
    }
    out
}

fn extract_xml_paragraphs(xml: &str, para_tag: &str, text_tag: &str) -> Vec<(bool, String)> {
    let mut paras = Vec::new();
    let open_prefix = format!("<{para_tag}");
    let close_tag = format!("</{para_tag}>");
    let mut rest = xml;
    while let Some(pos) = rest.find(&open_prefix) {
        let after = &rest[pos + open_prefix.len()..];
        let next = after.as_bytes().first().copied();
        if !matches!(next, Some(b'>' | b' ')) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else {
            break;
        };
        let inside = &after[gt + 1..];
        if after[..gt].ends_with('/') {
            rest = inside;
            continue;
        }
        if let Some(end) = inside.find(&close_tag) {
            let block = &inside[..end];
            let is_heading = block.contains("Heading")
                || block.contains("w:sz w:val=\"3")
                || block.contains("w:sz w:val=\"4");
            let text = extract_tag_runs(block, text_tag);
            if !text.trim().is_empty() {
                paras.push((is_heading, text.trim().to_owned()));
            }
            rest = &inside[end + close_tag.len()..];
        } else {
            break;
        }
    }
    paras
}

fn parse_csv_rows(csv: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in csv.lines() {
        let mut cols = Vec::new();
        let mut cur = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if in_quotes {
                if ch == '"' {
                    if chars.peek() == Some(&'"') {
                        cur.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    cur.push(ch);
                }
            } else if ch == '"' {
                in_quotes = true;
            } else if ch == ',' {
                cols.push(cur.trim().to_owned());
                cur.clear();
            } else {
                cur.push(ch);
            }
        }
        cols.push(cur.trim().to_owned());
        if cols.iter().any(|c| !c.is_empty()) {
            rows.push(cols);
        }
    }
    rows
}

fn col_index_from_ref(cell_ref: &str) -> Option<usize> {
    let letters: String = cell_ref
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if letters.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    for b in letters.bytes() {
        idx = idx * 26 + (b.to_ascii_uppercase() - b'A') as usize + 1;
    }
    Some(idx - 1)
}

fn parse_xlsx_attr(attrs: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let pos = attrs.find(&pat)?;
    let val_start = &attrs[pos + pat.len()..];
    let end = val_start.find('"')?;
    Some(val_start[..end].to_owned())
}

fn parse_xlsx_sheet_rows(xml: &str, shared: &[String]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut rest = xml;
    while let Some(r_pos) = rest.find("<row") {
        let after = &rest[r_pos + 4..];
        let Some(gt) = after.find('>') else { break };
        let inside_start = &after[gt + 1..];
        let Some(r_end) = inside_start.find("</row>") else {
            break;
        };
        let row_block = &inside_start[..r_end];
        rest = &inside_start[r_end + 6..];

        let mut row_cells: Vec<String> = Vec::new();
        let mut cr = row_block;
        while let Some(c_pos) = cr.find("<c") {
            let cafter = &cr[c_pos + 2..];
            if !matches!(cafter.as_bytes().first().copied(), Some(b'>' | b' ')) {
                cr = cafter;
                continue;
            }
            let Some(cgt) = cafter.find('>') else { break };
            let attrs = &cafter[..cgt];
            let c_inside = &cafter[cgt + 1..];
            let (cell_xml, next_cr) = if attrs.ends_with('/') {
                ("", c_inside)
            } else if let Some(cend) = c_inside.find("</c>") {
                (&c_inside[..cend], &c_inside[cend + 4..])
            } else {
                break;
            };
            cr = next_cr;
            let cell_type = parse_xlsx_attr(attrs, "t").unwrap_or_default();
            let value = if cell_type == "inlineStr" {
                extract_tag_runs(cell_xml, "t")
            } else {
                let raw_v = extract_tag_runs(cell_xml, "v");
                if cell_type == "s" {
                    raw_v
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| shared.get(i))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    raw_v
                }
            };
            let target_col = parse_xlsx_attr(attrs, "r")
                .and_then(|r| col_index_from_ref(&r))
                .unwrap_or(row_cells.len());
            if target_col < 64 {
                if row_cells.len() <= target_col {
                    row_cells.resize(target_col + 1, String::new());
                }
                row_cells[target_col] = value.trim().to_owned();
            }
        }
        if row_cells.iter().any(|c| !c.is_empty()) {
            rows.push(row_cells);
        }
    }
    rows
}

fn render_table_pdf(title: &str, sheets: &[(String, Vec<Vec<String>>)]) -> Vec<u8> {
    let mut layout = PdfLayout::custom(841.89, 595.28, 36.0, 36.0);
    let accent = (0.78, 0.16, 0.21);
    let ink = (0.12, 0.14, 0.18);
    let border = (0.82, 0.85, 0.89);

    for (s_idx, (sheet_name, rows)) in sheets.iter().enumerate() {
        if s_idx > 0 {
            layout.new_page();
        }
        layout.draw_wrapped(sheet_name, FontKind::Bold, 14.0, 0.0, accent, 1.3);
        layout.draw_rule(accent, 0.9);
        let col_count = rows.iter().map(Vec::len).max().unwrap_or(1).clamp(1, 10);
        let col_w = (layout.width - layout.margin_x * 2.0) / col_count as f32;
        let row_h = 18.0f32;

        for (r_idx, cols) in rows.iter().enumerate() {
            layout.ensure_space(row_h + 2.0);
            let y_bottom = layout.y - row_h;
            if r_idx == 0 {
                layout.fill_rect(
                    layout.margin_x,
                    y_bottom,
                    layout.width - layout.margin_x * 2.0,
                    row_h,
                    (0.95, 0.96, 0.98),
                );
            }
            for c_idx in 0..col_count {
                let x = layout.margin_x + c_idx as f32 * col_w;
                layout.stroke_rect(x, y_bottom, col_w, row_h, border, 0.4);
                let cell = cols.get(c_idx).map(String::as_str).unwrap_or("");
                let clipped = wrap_text(cell, (col_w - 8.0).max(20.0), 8.5, FontKind::Regular)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                let fk = if r_idx == 0 {
                    FontKind::Bold
                } else {
                    FontKind::Regular
                };
                layout.draw_text_at(&clipped, x + 4.0, y_bottom + 5.0, fk, 8.5, ink);
            }
            layout.y -= row_h;
        }
    }
    layout.finish_pdf(title)
}

fn office_to_pdf_for(tool: &str, input: &Path, dir: &Path) -> Res<PathBuf> {
    let ext = input
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let allowed: &[&str] = match tool {
        "word-to-pdf" => &["doc", "docx", "odt", "rtf", "txt"],
        "powerpoint-to-pdf" => &["ppt", "pptx", "odp"],
        "excel-to-pdf" => &["xls", "xlsx", "ods", "csv"],
        _ => &["doc", "docx", "ppt", "pptx", "xls", "xlsx", "odt"],
    };
    if !allowed.contains(&ext.as_str()) {
        return Err(format!("Supported office formats: {}", allowed.join(", ")));
    }
    let out = dir.join("result.pdf");
    if conversions::try_libreoffice_pdf(input, dir, &out).is_ok() {
        return Ok(out);
    }

    let ink = (0.14, 0.16, 0.20);
    let accent = (0.78, 0.16, 0.21);
    let muted = (0.42, 0.46, 0.52);

    let pdf_bytes = match ext.as_str() {
        "docx" | "odt" => {
            let file = File::open(input).map_err(|e| format!("Invalid {ext}: {e}"))?;
            let mut archive =
                ZipArchive::new(file).map_err(|e| format!("Invalid {ext} archive: {e}"))?;
            let entry = if ext == "docx" {
                "word/document.xml"
            } else {
                "content.xml"
            };
            let xml = read_zip_entry_capped(&mut archive, entry)
                .ok_or_else(|| format!("Missing {entry} in archive"))?;
            let p_tag = if ext == "docx" { "w:p" } else { "text:p" };
            let t_tag = if ext == "docx" { "w:t" } else { "text:span" };
            let paras = if ext == "docx" {
                extract_xml_paragraphs(&xml, p_tag, t_tag)
            } else {
                extract_xml_paragraphs(&xml, "text:p", "text:p")
            };
            let mut layout = PdfLayout::a4(50.0, 50.0);
            if paras.is_empty() {
                let fallback = extract_tag_runs(&xml, if ext == "docx" { "w:t" } else { "text:p" });
                layout.draw_wrapped(&fallback, FontKind::Regular, 10.5, 0.0, ink, 1.45);
            } else {
                for (is_h, text) in paras {
                    if is_h {
                        layout.y -= 4.0;
                        layout.draw_wrapped(&text, FontKind::Bold, 13.5, 0.0, accent, 1.35);
                    } else {
                        layout.draw_wrapped(&text, FontKind::Regular, 10.5, 0.0, ink, 1.45);
                        layout.y -= 4.0;
                    }
                }
            }
            layout.finish_pdf("Word Document")
        }
        "txt" | "rtf" => {
            let raw = fs::read_to_string(input).map_err(|e| format!("Read {ext}: {e}"))?;
            let clean = if ext == "rtf" { strip_rtf(&raw) } else { raw };
            let mut layout = PdfLayout::a4(50.0, 50.0);
            for line in clean.lines() {
                layout.draw_wrapped(line, FontKind::Regular, 10.5, 0.0, ink, 1.45);
            }
            layout.finish_pdf("Document")
        }
        "pptx" => {
            let file = File::open(input).map_err(|e| format!("Invalid pptx: {e}"))?;
            let mut archive =
                ZipArchive::new(file).map_err(|e| format!("Invalid pptx archive: {e}"))?;
            let mut slide_names: Vec<(usize, String)> = archive
                .file_names()
                .filter_map(|name| {
                    let rest = name
                        .strip_prefix("ppt/slides/slide")?
                        .strip_suffix(".xml")?;
                    let num = rest.parse::<usize>().ok()?;
                    Some((num, name.to_owned()))
                })
                .collect();
            slide_names.sort_by_key(|(n, _)| *n);
            if slide_names.is_empty() {
                return Err("No slides found inside PPTX".into());
            }
            let mut layout = PdfLayout::custom(720.0, 405.0, 44.0, 36.0);
            for (idx, (_, name)) in slide_names.iter().enumerate() {
                if idx > 0 {
                    layout.new_page();
                }
                layout.fill_rect(0.0, 399.0, 720.0, 6.0, accent);
                if let Some(xml) = read_zip_entry_capped(&mut archive, name) {
                    let paras = extract_xml_paragraphs(&xml, "a:p", "a:t");
                    for (p_idx, (_, text)) in paras.into_iter().enumerate() {
                        if p_idx == 0 {
                            layout.draw_wrapped(&text, FontKind::Bold, 20.0, 0.0, ink, 1.3);
                            layout.draw_rule(accent, 0.9);
                            layout.y -= 4.0;
                        } else {
                            layout.draw_wrapped(&text, FontKind::Regular, 12.5, 0.0, ink, 1.45);
                            layout.y -= 3.0;
                        }
                    }
                }
                layout.draw_text_at(
                    &format!("Slide {}", idx + 1),
                    640.0,
                    18.0,
                    FontKind::Regular,
                    8.5,
                    muted,
                );
            }
            layout.finish_pdf("Presentation")
        }
        "xlsx" => {
            let file = File::open(input).map_err(|e| format!("Invalid xlsx: {e}"))?;
            let mut archive =
                ZipArchive::new(file).map_err(|e| format!("Invalid xlsx archive: {e}"))?;
            let shared: Vec<String> = read_zip_entry_capped(&mut archive, "xl/sharedStrings.xml")
                .map(|xml| {
                    extract_xml_paragraphs(&xml, "si", "t")
                        .into_iter()
                        .map(|(_, s)| s)
                        .collect()
                })
                .unwrap_or_default();
            let mut sheet_names: Vec<(usize, String)> = archive
                .file_names()
                .filter_map(|name| {
                    let rest = name
                        .strip_prefix("xl/worksheets/sheet")?
                        .strip_suffix(".xml")?;
                    let num = rest.parse::<usize>().ok()?;
                    Some((num, name.to_owned()))
                })
                .collect();
            sheet_names.sort_by_key(|(n, _)| *n);
            if sheet_names.is_empty() {
                return Err("No worksheets found inside XLSX".into());
            }
            let mut sheets = Vec::new();
            for (n, name) in sheet_names {
                if let Some(xml) = read_zip_entry_capped(&mut archive, &name) {
                    let rows = parse_xlsx_sheet_rows(&xml, &shared);
                    if !rows.is_empty() {
                        sheets.push((format!("Sheet {n}"), rows));
                    }
                }
            }
            if sheets.is_empty() {
                return Err("Spreadsheet has no readable cells".into());
            }
            render_table_pdf("Spreadsheet", &sheets)
        }
        "csv" => {
            let raw = fs::read_to_string(input).map_err(|e| format!("Read csv: {e}"))?;
            let rows = parse_csv_rows(&raw);
            if rows.is_empty() {
                return Err("CSV file is empty".into());
            }
            render_table_pdf("CSV Table", &[("CSV Sheet".into(), rows)])
        }
        _ => {
            return Err(format!(
                "Legacy .{ext} conversion requires LibreOffice (soffice); save as OpenXML (.docx/.pptx/.xlsx) or install LibreOffice"
            ));
        }
    };

    fs::write(&out, pdf_bytes).map_err(|e| e.to_string())?;
    Ok(out)
}

fn strip_rtf(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    let mut skip_group = 0usize;
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if skip_group > 0 {
                    skip_group += 1;
                }
            }
            '}' => skip_group = skip_group.saturating_sub(1),
            '\\' => {
                let mut cmd = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphabetic() {
                        cmd.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '-' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                if matches!(cmd.as_str(), "fonttbl" | "colortbl" | "stylesheet" | "info") {
                    skip_group = 1;
                } else if cmd == "par" || cmd == "line" {
                    out.push('\n');
                }
            }
            _ if skip_group == 0 && !matches!(ch, '\r' | '\n') => out.push(ch),
            _ => {}
        }
    }
    out
}

pub(crate) fn pdf_to_markdown_fallback(input: &Path, dir: &Path, out: &Path) -> Res<()> {
    let pages = extract_pdf_pages_text(input, dir)?;
    if pages.iter().all(|p| p.trim().is_empty()) {
        return Err("No extractable text. Run OCR PDF first".into());
    }
    let mut md_pages = Vec::new();
    for (idx, page) in pages.iter().enumerate() {
        let mut lines = Vec::new();
        for (l_idx, line) in page.lines().enumerate() {
            let cols = split_table_columns(line);
            if cols.len() >= 2 {
                lines.push(format!("| {} |", cols.join(" | ")));
            } else {
                let t = line.trim();
                if !t.is_empty() {
                    if l_idx == 0 && t.len() < 100 && idx == 0 {
                        lines.push(format!("# {t}"));
                    } else {
                        lines.push(t.to_owned());
                    }
                }
            }
        }
        md_pages.push(lines.join("\n\n"));
    }
    fs::write(out, md_pages.join("\n\n---\n\n")).map_err(|e| e.to_string())
}

pub(crate) fn html_to_pdf_fallback(html: &str, out: &Path) -> Res<()> {
    if html.len() > 2_000_000 {
        return Err("HTML exceeds the 2 MB limit".into());
    }
    let plain = conversions::html_text(html);
    if plain.trim().is_empty() {
        return Err("HTML contains no visible text".into());
    }
    let mut layout = PdfLayout::a4(50.0, 50.0);
    let ink = (0.14, 0.16, 0.20);
    let mut first = true;
    for para in plain.lines() {
        let t = para.trim();
        if t.is_empty() {
            continue;
        }
        if first && t.len() < 120 {
            layout.draw_wrapped(t, FontKind::Bold, 15.0, 0.0, ink, 1.35);
            layout.draw_rule((0.78, 0.16, 0.21), 0.8);
            first = false;
        } else {
            layout.draw_wrapped(t, FontKind::Regular, 10.5, 0.0, ink, 1.45);
            layout.y -= 4.0;
        }
    }
    let pdf_bytes = layout.finish_pdf("HTML Document");
    fs::write(out, pdf_bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pdf(dir: &Path) -> PathBuf {
        let mut layout = PdfLayout::a4(50.0, 50.0);
        layout.draw_wrapped(
            "Quarterly Financial Summary",
            FontKind::Bold,
            16.0,
            0.0,
            (0.1, 0.1, 0.1),
            1.3,
        );
        let y1 = layout.y - 16.0;
        layout.draw_text_at("Revenue", 50.0, y1, FontKind::Bold, 11.0, (0.1, 0.1, 0.1));
        layout.draw_text_at("Expenses", 200.0, y1, FontKind::Bold, 11.0, (0.1, 0.1, 0.1));
        layout.draw_text_at("Profit", 350.0, y1, FontKind::Bold, 11.0, (0.1, 0.1, 0.1));
        let y2 = y1 - 18.0;
        layout.draw_text_at("1200", 50.0, y2, FontKind::Regular, 11.0, (0.1, 0.1, 0.1));
        layout.draw_text_at("450", 200.0, y2, FontKind::Regular, 11.0, (0.1, 0.1, 0.1));
        layout.draw_text_at("750", 350.0, y2, FontKind::Regular, 11.0, (0.1, 0.1, 0.1));
        layout.y = y2 - 18.0;
        let p = dir.join("sample.pdf");
        fs::write(&p, layout.finish_pdf("Sample")).unwrap();
        p
    }

    #[test]
    fn pdf_word_roundtrip_produces_valid_docx_and_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = sample_pdf(dir.path());
        let (docx, mime) = run(
            "pdf-to-word",
            std::slice::from_ref(&pdf),
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        assert!(doc_xml.contains("Quarterly Financial Summary"));

        let (back_pdf, back_mime) =
            run("word-to-pdf", &[docx], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(back_mime, "application/pdf");
        let loaded = Document::load(&back_pdf).unwrap();
        assert!(!loaded.get_pages().is_empty());

        let exact_opts = HashMap::from([("mode".into(), "exact".into())]);
        let (exact_docx, exact_mime) = run(
            "pdf-to-word",
            std::slice::from_ref(&pdf),
            &exact_opts,
            dir.path(),
        )
        .unwrap();
        assert_eq!(exact_mime, MIME_DOCX);
        let mut exact_arch = ZipArchive::new(File::open(&exact_docx).unwrap()).unwrap();
        let exact_xml = read_zip_entry_capped(&mut exact_arch, "word/document.xml").unwrap();
        assert!(exact_xml.contains("w:framePr"));
        assert!(exact_xml.contains("w:color"));
        assert!(exact_xml.contains("<w:b/>"));

        let flow_opts = HashMap::from([("mode".into(), "flow".into())]);
        let (flow_docx, flow_mime) = run("pdf-to-word", &[pdf], &flow_opts, dir.path()).unwrap();
        assert_eq!(flow_mime, MIME_DOCX);
        let mut flow_arch = ZipArchive::new(File::open(&flow_docx).unwrap()).unwrap();
        let flow_xml = read_zip_entry_capped(&mut flow_arch, "word/document.xml").unwrap();
        assert!(flow_xml.contains("Quarterly Financial Summary"));
        assert!(flow_xml.contains("<w:tbl>"));
    }

    #[test]
    fn pdf_word_exact_preserves_colored_banner_and_non_text_graphics() {
        let dir = tempfile::tempdir().unwrap();
        let mut layout = PdfLayout::a4(44.0, 44.0);
        layout.fill_rect(0.0, 740.0, 595.28, 101.89, (0.09, 0.14, 0.24));
        layout.draw_wrapped(
            "Nadia Kusuma",
            FontKind::Bold,
            22.0,
            0.0,
            (0.95, 0.25, 0.22),
            1.2,
        );
        layout.draw_wrapped(
            "Lead Architect",
            FontKind::Regular,
            11.0,
            0.0,
            (0.14, 0.16, 0.20),
            1.3,
        );
        layout.new_page();
        layout.draw_wrapped(
            "Page Two Heading",
            FontKind::Bold,
            18.0,
            0.0,
            (0.95, 0.25, 0.22),
            1.2,
        );
        let pdf = dir.path().join("styled.pdf");
        fs::write(&pdf, layout.finish_pdf("Styled")).unwrap();

        let exact_opts = HashMap::from([("mode".into(), "exact".into())]);
        let (docx, mime) = run("pdf-to-word", &[pdf], &exact_opts, dir.path()).unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        assert!(doc_xml.contains("Nadia Kusuma"));
        assert!(doc_xml.contains("Page Two Heading"));
        assert!(doc_xml.contains("behindDoc=\"1\""));
        assert_eq!(doc_xml.matches("<w:sectPr>").count(), 2);
        assert!(doc_xml.matches("w:color w:val=\"F2").count() >= 2);
        let media_count = arch
            .file_names()
            .filter(|name| name.starts_with("word/media/img"))
            .count();
        assert!(media_count >= 2);
    }

    #[test]
    fn pdf_word_auto_uses_pdf2docx_when_venv_present() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = sample_pdf(dir.path());
        let (docx, mime) = run("pdf-to-word", &[pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        for token in ["Revenue", "Expenses", "Profit", "1200", "450", "750"] {
            assert!(doc_xml.contains(token), "missing {token}");
        }
        if pdf2docx_python_candidates()
            .iter()
            .any(|p| p != "python3" && Path::new(p).is_file())
        {
            let core_xml =
                read_zip_entry_capped(&mut arch, "docProps/core.xml").unwrap_or_default();
            assert!(core_xml.contains("python-docx") || !doc_xml.contains("w:framePr"));
        }
    }

    #[test]
    fn pdf_word_auto_ocr_converts_scanned_image_pdf_to_editable_table() {
        if !pdf2docx_python_candidates()
            .iter()
            .any(|p| p != "python3" && Path::new(p).is_file())
        {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src_pdf = sample_pdf(dir.path());
        let scan_png = dir.path().join("scanned-page.png");
        conversions::command(
            "gs",
            &[
                "-q".into(),
                "-dSAFER".into(),
                "-dBATCH".into(),
                "-dNOPAUSE".into(),
                "-sDEVICE=png16m".into(),
                "-r200".into(),
                format!("-o{}", scan_png.display()),
                "-f".into(),
                src_pdf.display().to_string(),
            ],
            dir.path(),
        )
        .unwrap();
        let (scan_pdf, _) = crate::tools::run(
            "image-to-pdf",
            &[scan_png],
            &HashMap::from([
                ("page_size".into(), "a4".into()),
                ("margin".into(), "0".into()),
            ]),
            dir.path(),
        )
        .unwrap();

        let (docx, mime) = run("pdf-to-word", &[scan_pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        assert!(doc_xml.contains("Quarterly"));
        assert!(doc_xml.contains("Revenue"));
        assert!(doc_xml.contains("1200"));
        assert!(doc_xml.contains("<w:tbl"));
    }

    #[test]
    fn pdf_word_fixes_table_grids_heights_and_unwraps_photo_header() {
        if !pdf2docx_python_candidates()
            .iter()
            .any(|p| p != "python3" && Path::new(p).is_file())
        {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut layout = PdfLayout::a4(36.0, 36.0);
        layout.draw_text_at(
            "MUHAMMAD DANIEL KRISNA HALIM PUTRA",
            142.0,
            786.0,
            FontKind::Bold,
            17.5,
            (0.08, 0.08, 0.08),
        );
        layout.draw_text_at(
            "+6285156644103 | halimputra0701@gmail.com | https://linkedin.com/in/halimp/",
            142.0,
            770.0,
            FontKind::Regular,
            8.3,
            (0.45, 0.45, 0.45),
        );
        layout.draw_text_at(
            "Ko Asrama Polisi Blok N No.20, RT.2/RW.7, Ketintang, Gayungan, Surabaya",
            142.0,
            758.0,
            FontKind::Regular,
            8.3,
            (0.45, 0.45, 0.45),
        );
        layout.draw_text_at(
            "Full-stack developer with 3+ years of experience building scalable web applications.",
            142.0,
            740.0,
            FontKind::Regular,
            8.8,
            (0.15, 0.15, 0.15),
        );
        layout.draw_text_at(
            "Specialized in AI-powered SaaS solutions and enterprise software development.",
            142.0,
            728.0,
            FontKind::Regular,
            8.8,
            (0.15, 0.15, 0.15),
        );
        layout.draw_text_at(
            "Education Level",
            36.0,
            668.0,
            FontKind::Bold,
            14.0,
            (0.08, 0.08, 0.08),
        );
        layout.draw_text_at(
            "Institut Teknologi Sepuluh Nopember",
            36.0,
            644.0,
            FontKind::Bold,
            10.5,
            (0.1, 0.1, 0.1),
        );
        layout.draw_text_at(
            "2019 - 2023",
            455.0,
            644.0,
            FontKind::Regular,
            10.0,
            (0.35, 0.35, 0.35),
        );
        let base_pdf = dir.path().join("cv-base.pdf");
        fs::write(&base_pdf, layout.finish_pdf("CV")).unwrap();

        let img = image::RgbImage::from_pixel(120, 120, image::Rgb([70, 95, 130]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let ann = serde_json::json!([{
            "type": "image",
            "page": 1,
            "x": 36.0,
            "y": 716.0,
            "width": 90.0,
            "height": 90.0,
            "image_base64": format!("data:image/png;base64,{b64}")
        }])
        .to_string();
        let (cv_pdf, _) = crate::tools::run(
            "edit-pdf",
            &[base_pdf],
            &HashMap::from([("annotations".into(), ann)]),
            dir.path(),
        )
        .unwrap();

        let (docx, mime) = run("pdf-to-word", &[cv_pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        assert!(doc_xml.contains("MUHAMMAD DANIEL KRISNA HALIM PUTRA"));
        assert!(doc_xml.contains("development."));
        assert!(doc_xml.contains("Education Level"));
        assert!(doc_xml.contains("<wp:anchor"));
        assert!(!doc_xml.contains("w:hRule=\"exact\""));
        assert!(!doc_xml.contains("w:w=\"0.0\""));
        assert!(!doc_xml.contains("w:sz=\"8.0\""));
        assert!(!doc_xml.contains("w:gridCol w:w=\"5430\""));
    }

    #[test]
    fn pdf_powerpoint_roundtrip_editable_and_visual() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = sample_pdf(dir.path());
        let opts = HashMap::from([("mode".into(), "editable".into())]);
        let (pptx, mime) = run(
            "pdf-to-powerpoint",
            std::slice::from_ref(&pdf),
            &opts,
            dir.path(),
        )
        .unwrap();
        assert_eq!(mime, MIME_PPTX);
        let mut arch = ZipArchive::new(File::open(&pptx).unwrap()).unwrap();
        let slide = read_zip_entry_capped(&mut arch, "ppt/slides/slide1.xml").unwrap();
        assert!(slide.contains("Quarterly Financial Summary"));

        let (back_pdf, back_mime) =
            run("powerpoint-to-pdf", &[pptx], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(back_mime, "application/pdf");
        assert!(!Document::load(&back_pdf).unwrap().get_pages().is_empty());

        let (vis_pptx, vis_mime) =
            run("pdf-to-powerpoint", &[pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(vis_mime, MIME_PPTX);
        let vis_arch = ZipArchive::new(File::open(&vis_pptx).unwrap()).unwrap();
        assert!(vis_arch.len() >= 8);
    }

    #[test]
    fn pdf_excel_and_csv_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = sample_pdf(dir.path());
        let (xlsx, mime) = run("pdf-to-excel", &[pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(mime, MIME_XLSX);
        let mut arch = ZipArchive::new(File::open(&xlsx).unwrap()).unwrap();
        let sheet = read_zip_entry_capped(&mut arch, "xl/worksheets/sheet1.xml").unwrap();
        assert!(sheet.contains("Quarterly Financial Summary"));

        let (back_pdf, back_mime) =
            run("excel-to-pdf", &[xlsx], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(back_mime, "application/pdf");
        assert!(!Document::load(&back_pdf).unwrap().get_pages().is_empty());

        let csv = dir.path().join("data.csv");
        fs::write(
            &csv,
            "Name,Role,Score\nAlice,Engineer,98\nBob,Designer,94\n",
        )
        .unwrap();
        let (csv_pdf, _) = run("excel-to-pdf", &[csv], &HashMap::new(), dir.path()).unwrap();
        assert!(!Document::load(&csv_pdf).unwrap().get_pages().is_empty());
    }

    #[test]
    fn html_fallback_strips_scripts_and_renders_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("html.pdf");
        let html =
            "<h1>Invoice 520</h1><script>fetch('http://127.0.0.1')</script><p>Total: $99</p>";
        html_to_pdf_fallback(html, &out).unwrap();
        let bytes = fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(!bytes.windows(9).any(|w| w == b"127.0.0.1"));
    }
}
