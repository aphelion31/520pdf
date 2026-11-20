//! Local PDF operations. Coordinates are PDF points, measured from bottom-left.
use lopdf::{
    Dictionary, Document, Object, ObjectId, Stream,
    content::{Content, Operation},
    dictionary,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
type Options = HashMap<String, String>;

pub fn run(
    tool: &str,
    inputs: &[PathBuf],
    options: &Options,
    dir: &Path,
) -> Result<(PathBuf, String)> {
    if inputs.is_empty() {
        return Err("At least one input is required".into());
    }
    if inputs.len() > 100 {
        return Err("At most 100 inputs are supported".into());
    }
    let mut doc = if tool == "image-to-pdf" {
        images(inputs)?
    } else if tool == "merge" {
        if inputs.len() < 2 {
            return Err("Merge requires at least two PDFs".into());
        }
        merge(inputs)?
    } else {
        if inputs.len() != 1 {
            return Err("This tool requires exactly one PDF".into());
        }
        load(&inputs[0])?
    };
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err("PDF has no pages".into());
    }
    let selected = selection(options.get("pages").map(String::as_str), pages.len() as u32)?;
    match tool {
        "merge" | "image-to-pdf" => {}
        "split" | "organize" => {
            if !options.contains_key("pages") {
                return Err("pages is required, e.g. 1,3-5".into());
            }
            let ids: Vec<_> = selected.iter().map(|n| pages[n]).collect();
            rebuild_pages(&mut doc, &ids)?;
        }
        "rotate" => {
            let angle = integer(options, "angle", 90)?;
            if angle % 90 != 0 {
                return Err("angle must be a multiple of 90".into());
            }
            for n in selected {
                let id = pages[&n];
                let old = inherited(&doc, id, b"Rotate")?
                    .unwrap_or(0.into())
                    .as_i64()
                    .map_err(err)?;
                doc.get_dictionary_mut(id).map_err(err)?.set(
                    "Rotate",
                    (old.rem_euclid(360) + angle.rem_euclid(360)).rem_euclid(360),
                );
            }
        }
        "crop" => {
            let x = number(options, "x", 0.)?;
            let y = number(options, "y", 0.)?;
            let w = number(options, "width", 0.)?;
            let h = number(options, "height", 0.)?;
            if w <= 0. || h <= 0. {
                return Err("crop width and height must be positive PDF points".into());
            }
            for n in selected {
                let id = pages[&n];
                let bounds = page_box(&doc, id)?;
                if x < bounds[0] || y < bounds[1] || x + w > bounds[2] || y + h > bounds[3] {
                    return Err("Crop rectangle exceeds page MediaBox".into());
                }
                doc.get_dictionary_mut(id).map_err(err)?.set(
                    "CropBox",
                    vec![x.into(), y.into(), (x + w).into(), (y + h).into()],
                );
            }
        }
        "watermark" | "sign" | "page-numbers" => {
            let text = options.get("text").map(String::as_str).unwrap_or("");
            if tool != "page-numbers" && text.trim().is_empty() {
                return Err("text is required".into());
            }
            let size = number(
                options,
                "font_size",
                if tool == "page-numbers" { 12. } else { 32. },
            )?;
            if !(1. ..=300.).contains(&size) {
                return Err("font_size must be between 1 and 300".into());
            }
            let start = integer(options, "start", 1)?;
            if start < 0 || start > 1_000_000 {
                return Err("start must be between 0 and 1000000".into());
            }
            for (index, n) in selected.into_iter().enumerate() {
                let id = pages[&n];
                let bounds = page_box(&doc, id)?;
                let x = number(options, "x", bounds[0] + 36.)?;
                let y = number(
                    options,
                    "y",
                    if tool == "page-numbers" {
                        bounds[1] + 24.
                    } else {
                        (bounds[1] + bounds[3]) / 2.
                    },
                )?;
                if x < bounds[0] || x > bounds[2] || y < bounds[1] || y > bounds[3] {
                    return Err("Text position is outside the page".into());
                }
                let label = if tool == "page-numbers" {
                    (start + index as i64).to_string()
                } else {
                    text.to_string()
                };
                overlay(&mut doc, id, &label, x, y, size, tool == "watermark")?;
            }
        }
        "forms" => fill_forms(&mut doc, options)?,
        _ => return Err(format!("Unsupported PDF tool: {tool}")),
    }
    doc.prune_objects();
    doc.compress();
    let output = dir.join(format!("{tool}.pdf"));
    if inputs.iter().any(|input| {
        input == &output
            || (input.canonicalize().ok().is_some()
                && input.canonicalize().ok() == output.canonicalize().ok())
    }) {
        return Err("Output would overwrite an input. Use a separate output directory".into());
    }
    doc.save(&output).map_err(err)?;
    Ok((output, "application/pdf".into()))
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn load(path: &Path) -> Result<Document> {
    let doc = Document::load(path).map_err(|e| format!("Invalid PDF: {e}"))?;
    if doc.is_encrypted() {
        return Err("Encrypted PDF: unlock it before editing".into());
    }
    if doc.objects.values().any(|o| {
        o.as_dict()
            .ok()
            .is_some_and(|d| d.get(b"Type").ok().and_then(|o| o.as_name().ok()) == Some(b"Sig"))
    }) {
        return Err(
            "Editing digitally signed PDFs invalidates signatures. Use an unsigned copy".into(),
        );
    }
    Ok(doc)
}
fn number(options: &Options, key: &str, default: f32) -> Result<f32> {
    let n = match options.get(key) {
        Some(v) => v
            .parse::<f32>()
            .map_err(|_| format!("{key} must be numeric"))?,
        None => default,
    };
    if !n.is_finite() || n.abs() > 1_000_000. {
        return Err(format!("{key} is out of range"));
    }
    Ok(n)
}
fn integer(options: &Options, key: &str, default: i64) -> Result<i64> {
    options
        .get(key)
        .map(|v| {
            v.parse::<i64>()
                .map_err(|_| format!("{key} must be an integer"))
        })
        .unwrap_or(Ok(default))
}
fn selection(value: Option<&str>, count: u32) -> Result<Vec<u32>> {
    let Some(value) = value else {
        return Ok((1..=count).collect());
    };
    if value.len() > 20_000 {
        return Err("Page selection is too long".into());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for part in value.split(',') {
        let part = part.trim();
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let a = a
            .trim()
            .parse::<u32>()
            .map_err(|_| "Invalid page selection")?;
        let b = b
            .trim()
            .parse::<u32>()
            .map_err(|_| "Invalid page selection")?;
        if a == 0 || a > b || b > count {
            return Err(format!("Page selection must be within 1-{count}"));
        }
        for n in a..=b {
            if !seen.insert(n) {
                return Err("Duplicate pages are not supported".into());
            }
            out.push(n);
        }
    }
    if out.is_empty() {
        return Err("Select at least one page".into());
    }
    Ok(out)
}
fn inherited(doc: &Document, id: ObjectId, key: &[u8]) -> Result<Option<Object>> {
    let mut current = id;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current) {
            return Err("Cyclic PDF page inheritance".into());
        }
        let d = doc.get_dictionary(current).map_err(err)?;
        if let Ok(v) = d.get(key) {
            return Ok(Some(doc.dereference(v).map_err(err)?.1.clone()));
        }
        match d.get(b"Parent").and_then(Object::as_reference) {
            Ok(id) => current = id,
            Err(_) => return Ok(None),
        }
    }
}
fn page_box(doc: &Document, id: ObjectId) -> Result<[f32; 4]> {
    let obj = inherited(doc, id, b"MediaBox")?.ok_or("Missing MediaBox")?;
    let v = obj.as_array().map_err(err)?;
    if v.len() != 4 {
        return Err("Invalid MediaBox".into());
    }
    let mut b = [0.; 4];
    for (i, o) in v.iter().enumerate() {
        b[i] = o.as_float().map_err(err)?;
    }
    Ok(b)
}
fn materialize(doc: &mut Document, id: ObjectId) -> Result<()> {
    for key in [b"MediaBox".as_slice(), b"CropBox", b"Resources", b"Rotate"] {
        if let Some(v) = inherited(doc, id, key)? {
            doc.get_dictionary_mut(id).map_err(err)?.set(key, v);
        }
    }
    Ok(())
}
fn rebuild_pages(doc: &mut Document, ids: &[ObjectId]) -> Result<()> {
    for &id in ids {
        materialize(doc, id)?;
    }
    let keep: HashSet<_> = ids.iter().copied().collect();
    let removed: Vec<_> = doc
        .get_pages()
        .into_values()
        .filter(|id| !keep.contains(id))
        .collect();
    for id in removed {
        doc.objects.remove(&id);
    }
    let root = doc.add_object(dictionary! {"Type"=>"Pages", "Kids"=>ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(), "Count"=>ids.len() as i64});
    for &id in ids {
        doc.get_dictionary_mut(id).map_err(err)?.set("Parent", root);
    }
    let catalog = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(err)?;
    doc.get_dictionary_mut(catalog)
        .map_err(err)?
        .set("Pages", root);
    // ponytail: document outlines cannot safely retain references after page removal.
    doc.get_dictionary_mut(catalog)
        .map_err(err)?
        .remove(b"Outlines");
    Ok(())
}
fn merge(inputs: &[PathBuf]) -> Result<Document> {
    let mut out = Document::with_version("1.7");
    let catalog = out.add_object(dictionary! {"Type"=>"Catalog"});
    out.trailer.set("Root", catalog);
    let mut pages = Vec::new();
    for path in inputs {
        let mut doc = load(path)?;
        let root = doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(err)?;
        if doc.get_dictionary(root).map_err(err)?.has(b"AcroForm") {
            return Err("Merge of interactive forms is unsupported. Flatten forms first".into());
        }
        for id in doc.get_pages().into_values() {
            materialize(&mut doc, id)?;
        }
        doc.renumber_objects_with(out.max_id + 1);
        pages.extend(doc.get_pages().into_values());
        out.max_id = doc.max_id;
        out.objects.extend(doc.objects);
    }
    rebuild_pages(&mut out, &pages)?;
    Ok(out)
}
fn latin_text(text: &str) -> Result<Vec<u8>> {
    if text.is_empty()
        || text.len() > 4096
        || !text.chars().all(|c| c == ' ' || ('!'..='~').contains(&c))
    {
        return Err(
            "Text must contain 1-4096 printable ASCII characters. Unicode requires embedded fonts"
                .into(),
        );
    }
    Ok(text.as_bytes().to_vec())
}
fn text_content(text: &str, x: f32, y: f32, size: f32, gray: f32) -> Result<Vec<u8>> {
    Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new("g", vec![gray.into()]),
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"ToolFont".to_vec()), size.into()]),
            Operation::new("Td", vec![x.into(), y.into()]),
            Operation::new("Tj", vec![Object::string_literal(latin_text(text)?)]),
            Operation::new("ET", vec![]),
            Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(err)
}
fn overlay(
    doc: &mut Document,
    id: ObjectId,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    light: bool,
) -> Result<()> {
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    // A Form XObject isolates our resource names from the original PDF's font names.
    let bounds = page_box(doc, id)?;
    let form=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>bounds.iter().copied().map(Object::Real).collect::<Vec<_>>(),"Resources"=>dictionary!{"Font"=>dictionary!{"ToolFont"=>font}}},text_content(text,x,y,size,if light {0.65} else {0.})?));
    let mut resources = inherited(doc, id, b"Resources")?
        .unwrap_or(Object::Dictionary(Dictionary::new()))
        .as_dict()
        .map_err(err)?
        .clone();
    let mut xobjects = match resources.get(b"XObject") {
        Ok(o) => doc
            .dereference(o)
            .map_err(err)?
            .1
            .as_dict()
            .map_err(err)?
            .clone(),
        Err(_) => Dictionary::new(),
    };
    let name = format!("ToolOverlay{}", form.0);
    xobjects.set(name.as_bytes(), form);
    resources.set("XObject", xobjects);
    doc.get_dictionary_mut(id)
        .map_err(err)?
        .set("Resources", resources);
    // Separate graphics state from malformed/unbalanced original content where possible.
    let prefix = doc.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
    let mut contents = vec![Object::Reference(prefix)];
    for c in doc.get_page_contents(id) {
        contents.push(c.into());
    }
    let suffix = doc.add_object(Stream::new(
        dictionary! {},
        format!("\nQ\nq\n/{name} Do\nQ\n").into_bytes(),
    ));
    contents.push(suffix.into());
    doc.get_dictionary_mut(id)
        .map_err(err)?
        .set("Contents", contents);
    Ok(())
}
fn fill_forms(doc: &mut Document, options: &Options) -> Result<()> {
    let requested: HashMap<_, _> = options
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("field.").map(|k| (k.to_string(), v.clone())))
        .collect();
    if requested.is_empty() {
        return Err("Supply text fields as field.<name>=value".into());
    }
    let mut found = HashSet::new();
    let ids: Vec<_> = doc.objects.keys().copied().collect();
    for id in ids {
        let d = match doc.get_dictionary(id) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let name = match d.get(b"T").and_then(Object::as_str) {
            Ok(v) => String::from_utf8_lossy(v).to_string(),
            Err(_) => continue,
        };
        let Some(value) = requested.get(&name) else {
            continue;
        };
        if inherited(doc, id, b"FT")?
            .and_then(|v| v.as_name().ok().map(|v| v.to_vec()))
            .as_deref()
            != Some(b"Tx")
        {
            return Err(format!("Field {name} is not a supported text field"));
        }
        let flags = inherited(doc, id, b"Ff")?
            .and_then(|v| v.as_i64().ok())
            .unwrap_or(0);
        if flags & (1 | 4096 | 8192 | 1048576 | 33554432) != 0 {
            return Err(format!(
                "Field {name} is read-only or has unsupported text formatting"
            ));
        }
        if !value.is_empty() {
            latin_text(value)?;
        }
        if let Some(max) = inherited(doc, id, b"MaxLen")?.and_then(|v| v.as_i64().ok()) {
            if value.len() as i64 > max {
                return Err(format!("Field {name} exceeds its maximum length"));
            }
        }
        let widgets = match doc.get_dictionary(id).map_err(err)?.get(b"Kids") {
            Ok(kids) => kids
                .as_array()
                .map_err(err)?
                .iter()
                .map(|o| o.as_reference().map_err(err))
                .collect::<Result<Vec<_>>>()?,
            Err(_) => vec![id],
        };
        for widget in widgets {
            field_appearance(doc, widget, value)?;
        }
        doc.get_dictionary_mut(id)
            .map_err(err)?
            .set("V", Object::string_literal(value.as_bytes()));
        found.insert(name);
    }
    if let Some(name) = requested.keys().find(|n| !found.contains(*n)) {
        return Err(format!("Text field not found: {name}"));
    }
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(err)?;
    let form = doc
        .get_dictionary(root)
        .map_err(err)?
        .get(b"AcroForm")
        .map_err(err)?
        .clone();
    match form {
        Object::Reference(id) => {
            doc.get_dictionary_mut(id)
                .map_err(err)?
                .set("NeedAppearances", false);
        }
        Object::Dictionary(mut d) => {
            d.set("NeedAppearances", false);
            doc.get_dictionary_mut(root)
                .map_err(err)?
                .set("AcroForm", d);
        }
        _ => return Err("Invalid AcroForm".into()),
    }
    Ok(())
}
fn field_appearance(doc: &mut Document, id: ObjectId, value: &str) -> Result<()> {
    let rect = inherited(doc, id, b"Rect")?.ok_or("Text field has no widget rectangle")?;
    let rect = rect.as_array().map_err(err)?;
    if rect.len() != 4 {
        return Err("Invalid field widget rectangle".into());
    }
    let width = rect[2].as_float().map_err(err)? - rect[0].as_float().map_err(err)?;
    let height = rect[3].as_float().map_err(err)? - rect[1].as_float().map_err(err)?;
    if !width.is_finite() || !height.is_finite() || width <= 4. || height <= 4. {
        return Err("Invalid field widget size".into());
    }
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let size = 12.0_f32
        .min(height - 4.)
        .min((width - 4.) / (value.len().max(1) as f32));
    let bytes = if value.is_empty() {
        Vec::new()
    } else {
        text_content(value, 2., (height - size) / 2., size, 0.)?
    };
    let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),width.into(),height.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"ToolFont"=>font}}},bytes));
    doc.get_dictionary_mut(id)
        .map_err(err)?
        .set("AP", dictionary! {"N"=>ap});
    Ok(())
}
fn images(inputs: &[PathBuf]) -> Result<Document> {
    let mut doc = Document::with_version("1.7");
    let parent = doc.new_object_id();
    let mut pages = Vec::new();
    let mut total_pixels = 0_u64;
    for path in inputs {
        let mut reader = image::ImageReader::open(path)
            .map_err(err)?
            .with_guessed_format()
            .map_err(err)?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12000);
        limits.max_image_height = Some(12000);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let rgba = reader
            .decode()
            .map_err(|e| format!("Invalid or oversized image: {e}"))?
            .to_rgba8();
        let (w, h) = rgba.dimensions();
        total_pixels += u64::from(w) * u64::from(h);
        if w == 0 || h == 0 || total_pixels > 40_000_000 {
            return Err("Combined images exceed 40 million pixels".into());
        }
        let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
        for p in rgba.pixels() {
            for c in &p.0[..3] {
                rgb.push(
                    ((u32::from(*c) * u32::from(p[3]) + 255 * (255 - u32::from(p[3])) + 127) / 255)
                        as u8,
                );
            }
        }
        let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},rgb));
        let scale = (595. / w as f32).min(842. / h as f32);
        let pw = w as f32 * scale;
        let ph = h as f32 * scale;
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("q {pw} 0 0 {ph} 0 0 cm /Image Do Q").into_bytes(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),pw.into(),ph.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Image"=>image}},"Contents"=>content});
        pages.push(page.into());
    }
    doc.objects.insert(
        parent,
        dictionary! {"Type"=>"Pages","Count"=>pages.len() as i64,"Kids"=>pages}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>parent});
    doc.trailer.set("Root", root);
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Document {
        let mut d = Document::with_version("1.7");
        let root = d.add_object(dictionary! {"Type"=>"Catalog"});
        d.trailer.set("Root", root);
        let parent = d.add_object(
            dictionary! {"Type"=>"Pages","MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()]},
        );
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(d.add_object(dictionary! {"Type"=>"Page","Parent"=>parent}));
        }
        d.get_dictionary_mut(parent).unwrap().set(
            "Kids",
            ids.iter()
                .copied()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
        d.get_dictionary_mut(parent).unwrap().set("Count", 3);
        d.get_dictionary_mut(root).unwrap().set("Pages", parent);
        d
    }
    #[test]
    fn selections_validate_and_preserve_order() {
        assert_eq!(selection(Some("3,1-2"), 3).unwrap(), vec![3, 1, 2]);
        for s in ["0", "4", "2-1", "1,1", "", "1-99999999"] {
            assert!(selection(Some(s), 3).is_err(), "{s}");
        }
    }
    #[test]
    fn reorder_preserves_inherited_boxes() {
        let mut d = fixture();
        let p = d.get_pages();
        rebuild_pages(&mut d, &[p[&3], p[&1]]).unwrap();
        assert_eq!(
            d.get_pages().into_values().collect::<Vec<_>>(),
            vec![p[&3], p[&1]]
        );
        assert_eq!(page_box(&d, p[&3]).unwrap(), [0., 0., 600., 800.]);
    }
    #[test]
    fn overlays_are_real_streams() {
        let mut d = fixture();
        let id = d.get_pages()[&1];
        overlay(&mut d, id, "Visible signature", 20., 30., 14., false).unwrap();
        let bytes = d.get_page_content(id);
        assert!(String::from_utf8_lossy(&bytes).contains("Do"));
        assert!(
            d.objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .any(|s| String::from_utf8_lossy(&s.content).contains("Visible signature"))
        );
        assert!(overlay(&mut d, id, "中文", 0., 0., 12., false).is_err());
    }
    #[test]
    fn operations_roundtrip_real_files() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        fixture().save(&input).unwrap();
        let options = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Options>()
        };
        let (rotated, _) = run(
            "rotate",
            &[input.clone()],
            &options(&[("pages", "2"), ("angle", "90")]),
            dir.path(),
        )
        .unwrap();
        let d = Document::load(rotated).unwrap();
        assert_eq!(
            inherited(&d, d.get_pages()[&2], b"Rotate")
                .unwrap()
                .unwrap()
                .as_i64()
                .unwrap(),
            90
        );
        assert!(
            inherited(&d, d.get_pages()[&1], b"Rotate")
                .unwrap()
                .is_none()
        );
        let (split, _) = run(
            "split",
            &[input.clone()],
            &options(&[("pages", "3,1")]),
            dir.path(),
        )
        .unwrap();
        assert_eq!(Document::load(split).unwrap().get_pages().len(), 2);
        let (merged, _) = run(
            "merge",
            &[input.clone(), input.clone()],
            &Options::new(),
            dir.path(),
        )
        .unwrap();
        assert_eq!(Document::load(merged).unwrap().get_pages().len(), 6);
        let (cropped, _) = run(
            "crop",
            &[input.clone()],
            &options(&[("width", "200"), ("height", "300")]),
            dir.path(),
        )
        .unwrap();
        let d = Document::load(cropped).unwrap();
        assert_eq!(
            inherited(&d, d.get_pages()[&1], b"CropBox")
                .unwrap()
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            4
        );
        for tool in ["watermark", "sign", "page-numbers"] {
            let (file, _) = run(
                tool,
                &[input.clone()],
                &options(&[("text", "CONFIDENTIAL")]),
                dir.path(),
            )
            .unwrap();
            let d = Document::load(file).unwrap();
            assert!(String::from_utf8_lossy(&d.get_page_content(d.get_pages()[&1])).contains("Do"));
        }
        assert!(
            run(
                "rotate",
                &[input.clone()],
                &options(&[("angle", "45")]),
                dir.path()
            )
            .is_err()
        );
        assert!(run("redact", &[input], &Options::new(), dir.path()).is_err());
    }
    #[test]
    fn image_to_pdf_embeds_real_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("pixels.png");
        image::RgbaImage::from_pixel(8, 4, image::Rgba([255, 0, 0, 128]))
            .save(&input)
            .unwrap();
        let (file, _) = run("image-to-pdf", &[input], &Options::new(), dir.path()).unwrap();
        let d = Document::load(file).unwrap();
        assert_eq!(d.get_pages().len(), 1);
        let s = d
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .find(|s| s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Image"))
            .unwrap();
        let pixels = s.decompressed_content().unwrap();
        assert_eq!(&pixels[..3], &[255, 127, 127]);
    }
    #[test]
    fn form_value_and_appearance_are_updated() {
        let mut d = fixture();
        let field=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("name"),"Rect"=>vec![0.into(),0.into(),200.into(),30.into()]});
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root).unwrap().set(
            "AcroForm",
            dictionary! {"Fields"=>vec![Object::Reference(field)]},
        );
        fill_forms(
            &mut d,
            &HashMap::from([("field.name".into(), "Ada Lovelace".into())]),
        )
        .unwrap();
        assert_eq!(
            d.get_dictionary(field)
                .unwrap()
                .get(b"V")
                .unwrap()
                .as_str()
                .unwrap(),
            b"Ada Lovelace"
        );
        let ap = d
            .get_dictionary(field)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&d.get_object(ap).unwrap().as_stream().unwrap().content)
                .contains("Ada Lovelace")
        );
        assert!(
            fill_forms(
                &mut d,
                &HashMap::from([("field.unknown".into(), "x".into())])
            )
            .is_err()
        );
    }
    #[test]
    fn numeric_validation() {
        assert!(number(&HashMap::from([("x".into(), "NaN".into())]), "x", 0.).is_err());
    }
}
