//! Bidirectional Office/OpenXML and offline document converters (DOCX, PPTX, XLSX, PDF, HTML, Markdown).
use crate::features::{
    conversions,
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
        "pdf-to-word" => pdf_to_word(&input, &dir).map(|p| (p, MIME_DOCX.into())),
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

fn pdf_to_word(input: &Path, dir: &Path) -> Res<PathBuf> {
    let pages = extract_pdf_pages_text(input, dir)?;
    if pages.iter().all(|p| p.trim().is_empty()) {
        return Err("No extractable text found in PDF. Run OCR PDF first.".into());
    }
    let out = dir.join("result.docx");
    let file = File::create(&out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let write_xml = |z: &mut ZipWriter<File>, name: &str, body: &str| -> Res<()> {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };

    write_xml(
        &mut zip,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#,
    )?;

    write_xml(
        &mut zip,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )?;

    write_xml(
        &mut zip,
        "word/_rels/document.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )?;

    write_xml(
        &mut zip,
        "word/styles.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:sz w:val="22"/></w:rPr>
  </w:style>
</w:styles>"#,
    )?;

    let mut body = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>"#,
    );

    for (idx, page_text) in pages.iter().enumerate() {
        if idx > 0 {
            body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
        }
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
    }

    body.push_str(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>"#,
    );
    write_xml(&mut zip, "word/document.xml", &body)?;
    zip.finish().map_err(|e| e.to_string())?;
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
        layout.draw_wrapped(
            "Revenue   Expenses   Profit",
            FontKind::Regular,
            11.0,
            0.0,
            (0.1, 0.1, 0.1),
            1.3,
        );
        layout.draw_wrapped(
            "1200      450        750",
            FontKind::Regular,
            11.0,
            0.0,
            (0.1, 0.1, 0.1),
            1.3,
        );
        let p = dir.join("sample.pdf");
        fs::write(&p, layout.finish_pdf("Sample")).unwrap();
        p
    }

    #[test]
    fn pdf_word_roundtrip_produces_valid_docx_and_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = sample_pdf(dir.path());
        let (docx, mime) = run("pdf-to-word", &[pdf], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(mime, MIME_DOCX);
        let mut arch = ZipArchive::new(File::open(&docx).unwrap()).unwrap();
        let doc_xml = read_zip_entry_capped(&mut arch, "word/document.xml").unwrap();
        assert!(doc_xml.contains("Quarterly Financial Summary"));

        let (back_pdf, back_mime) =
            run("word-to-pdf", &[docx], &HashMap::new(), dir.path()).unwrap();
        assert_eq!(back_mime, "application/pdf");
        let loaded = Document::load(&back_pdf).unwrap();
        assert!(!loaded.get_pages().is_empty());
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
