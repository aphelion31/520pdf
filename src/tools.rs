//! Local PDF operations. Coordinates are PDF points, measured from bottom-left.
use base64::Engine as _;
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
        images(inputs, options)?
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
    if tool == "forms" && options.get("action").is_some_and(|v| v == "discover") {
        let mut fields = Vec::new();
        for (&id, object) in &doc.objects {
            let Ok(dict) = object.as_dict() else { continue };
            let Ok(name) = dict.get(b"T").and_then(Object::as_str) else {
                continue;
            };
            let kind = inherited(&doc, id, b"FT")?.and_then(|v| {
                v.as_name()
                    .ok()
                    .map(|v| String::from_utf8_lossy(v).into_owned())
            });
            if let Some(kind) = kind {
                fields.push(serde_json::json!({"name":String::from_utf8_lossy(name),"type":kind}));
            }
        }
        fields.sort_by_key(|f| f["name"].as_str().unwrap_or("").to_owned());
        let output = dir.join("forms.json");
        if inputs.iter().any(|i| {
            i == &output
                || (i.canonicalize().ok().is_some()
                    && i.canonicalize().ok() == output.canonicalize().ok())
        }) {
            return Err("Output would overwrite an input".into());
        }
        std::fs::write(&output, serde_json::to_vec(&fields).map_err(err)?).map_err(err)?;
        return Ok((output, "application/json".into()));
    }
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
            let rotations: HashMap<String, i64> = match options.get("rotations") {
                Some(v) => serde_json::from_str(v)
                    .map_err(|_| "rotations must be a JSON object mapping pages to angles")?,
                None => HashMap::new(),
            };
            for key in rotations.keys() {
                let n: u32 = key.parse().map_err(|_| "Invalid rotation page")?;
                if !selected.contains(&n) {
                    return Err("Rotation page must be selected".into());
                }
            }
            for n in selected {
                let page_angle = rotations.get(&n.to_string()).copied().unwrap_or(angle);
                if page_angle % 90 != 0 {
                    return Err("rotation angles must be multiples of 90".into());
                }
                let id = pages[&n];
                let old = inherited(&doc, id, b"Rotate")?
                    .unwrap_or(0.into())
                    .as_i64()
                    .map_err(err)?;
                doc.get_dictionary_mut(id).map_err(err)?.set(
                    "Rotate",
                    (old.rem_euclid(360) + page_angle.rem_euclid(360)).rem_euclid(360),
                );
            }
        }
        "crop" => {
            let margins = options.contains_key("margin_top")
                || options.contains_key("margin_bottom")
                || options.contains_key("margin_left")
                || options.contains_key("margin_right");
            let x = number(options, "x", 0.)?;
            let y = number(options, "y", 0.)?;
            let w = number(options, "width", 0.)?;
            let h = number(options, "height", 0.)?;
            if !margins && (w <= 0. || h <= 0.) {
                return Err("crop width and height must be positive PDF points".into());
            }
            for n in selected {
                let id = pages[&n];
                let bounds = page_box(&doc, id)?;
                let (x, y, w, h) = if margins {
                    let left = number(options, "margin_left", 0.)?;
                    let right = number(options, "margin_right", 0.)?;
                    let top = number(options, "margin_top", 0.)?;
                    let bottom = number(options, "margin_bottom", 0.)?;
                    if [left, right, top, bottom].iter().any(|v| *v < 0.) {
                        return Err("Crop margins must be nonnegative".into());
                    }
                    (
                        bounds[0] + left,
                        bounds[1] + bottom,
                        bounds[2] - bounds[0] - left - right,
                        bounds[3] - bounds[1] - bottom - top,
                    )
                } else {
                    (x, y, w, h)
                };
                if w <= 0.
                    || h <= 0.
                    || x < bounds[0]
                    || y < bounds[1]
                    || x + w > bounds[2]
                    || y + h > bounds[3]
                {
                    return Err("Crop rectangle exceeds page MediaBox".into());
                }
                doc.get_dictionary_mut(id).map_err(err)?.set(
                    "CropBox",
                    vec![x.into(), y.into(), (x + w).into(), (y + h).into()],
                );
            }
        }
        "watermark" | "sign" | "page-numbers" => {
            if tool == "sign" && options.contains_key("image_base64") {
                for n in selected {
                    image_signature(&mut doc, pages[&n], options)?;
                }
            } else {
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
                    let position = options
                        .get("position")
                        .map(String::as_str)
                        .unwrap_or("left");
                    let default_x = match position {
                        "left" => bounds[0] + 36.,
                        "center" => (bounds[0] + bounds[2]) / 2.,
                        "right" => bounds[2] - 36.,
                        _ => return Err("position must be left, center or right".into()),
                    };
                    let x = number(options, "x", default_x)?;
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
                        options
                            .get("format")
                            .map(String::as_str)
                            .unwrap_or("{n}")
                            .replace("{n}", &(start + index as i64).to_string())
                            .replace("{total}", &pages.len().to_string())
                    } else {
                        text.to_string()
                    };
                    let rotation = number(options, "rotation", 0.)?;
                    let opacity = number(options, "opacity", 1.)?;
                    if !(0. ..=1.).contains(&opacity) {
                        return Err("opacity must be between 0 and 1".into());
                    }
                    overlay(
                        &mut doc,
                        id,
                        &label,
                        x,
                        y,
                        size,
                        tool == "watermark",
                        rotation,
                        opacity,
                    )?;
                }
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
    rotation: f32,
    opacity: f32,
) -> Result<()> {
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    // A Form XObject isolates our resource names from the original PDF's font names.
    let bounds = page_box(doc, id)?;
    let radians = rotation.to_radians();
    let (sin, cos) = radians.sin_cos();
    let mut content =
        format!("q /ToolOpacity gs {cos} {sin} {} {cos} {x} {y} cm\n", -sin).into_bytes();
    content.extend(text_content(
        text,
        0.,
        0.,
        size,
        if light { 0.65 } else { 0. },
    )?);
    content.extend_from_slice(b"\nQ");
    let form=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>bounds.iter().copied().map(Object::Real).collect::<Vec<_>>(),"Resources"=>dictionary!{"Font"=>dictionary!{"ToolFont"=>font},"ExtGState"=>dictionary!{"ToolOpacity"=>dictionary!{"Type"=>"ExtGState","ca"=>opacity,"CA"=>opacity}}}},content));
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
fn image_signature(doc: &mut Document, id: ObjectId, options: &Options) -> Result<()> {
    let encoded = options
        .get("image_base64")
        .ok_or("Missing signature image")?;
    if encoded.len() > 8_000_000 {
        return Err("Signature image exceeds size limit".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Invalid signature image base64")?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(err)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4000);
    limits.max_image_height = Some(4000);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader.decode().map_err(err)?.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 8_000_000 {
        return Err("Invalid signature image dimensions".into());
    }
    let bounds = page_box(doc, id)?;
    let x = number(options, "x", bounds[0] + 36.)?;
    let y = number(options, "y", bounds[1] + 36.)?;
    let width = number(options, "width", 150.)?;
    let height = number(options, "height", width * h as f32 / w as f32)?;
    if width <= 0.
        || height <= 0.
        || x < bounds[0]
        || y < bounds[1]
        || x + width > bounds[2]
        || y + height > bounds[3]
    {
        return Err("Signature rectangle exceeds page MediaBox".into());
    }
    let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
    let mut alpha = Vec::with_capacity(w as usize * h as usize);
    for p in rgba.pixels() {
        rgb.extend_from_slice(&p.0[..3]);
        alpha.push(p.0[3]);
    }
    let mask = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8}, alpha));
    let image = doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"SMask"=>mask}, rgb));
    let mut resources = inherited(doc, id, b"Resources")?
        .unwrap_or(Object::Dictionary(Dictionary::new()))
        .as_dict()
        .map_err(err)?
        .clone();
    let mut objects = match resources.get(b"XObject") {
        Ok(o) => doc
            .dereference(o)
            .map_err(err)?
            .1
            .as_dict()
            .map_err(err)?
            .clone(),
        Err(_) => Dictionary::new(),
    };
    let name = format!("Signature{}", image.0);
    objects.set(name.as_bytes(), image);
    resources.set("XObject", objects);
    doc.get_dictionary_mut(id)
        .map_err(err)?
        .set("Resources", resources);
    let prefix = doc.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
    let mut contents = vec![Object::Reference(prefix)];
    contents.extend(doc.get_page_contents(id).into_iter().map(Object::Reference));
    let suffix = doc.add_object(Stream::new(
        dictionary! {},
        format!("\nQ\nq {width} 0 0 {height} {x} {y} cm /{name} Do Q\n").into_bytes(),
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
        let kind = inherited(doc, id, b"FT")?.and_then(|v| v.as_name().ok().map(|v| v.to_vec()));
        if kind.as_deref() == Some(b"Btn") {
            let flags = inherited(doc, id, b"Ff")?
                .and_then(|v| v.as_i64().ok())
                .unwrap_or(0);
            if flags & (1 | 32768 | 65536) != 0 {
                return Err(format!("Field {name} is read-only or not a checkbox"));
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
            let mut state = None;
            for widget in &widgets {
                let d = doc.get_dictionary(*widget).map_err(err)?;
                let ap = d
                    .get(b"AP")
                    .map_err(|_| format!("Checkbox {name} has no appearance"))?;
                let ap = doc.dereference(ap).map_err(err)?.1.as_dict().map_err(err)?;
                let normal = doc
                    .dereference(ap.get(b"N").map_err(err)?)
                    .map_err(err)?
                    .1
                    .as_dict()
                    .map_err(err)?;
                let on = normal
                    .iter()
                    .find(|(k, _)| k.as_slice() != b"Off")
                    .map(|(k, _)| k.clone())
                    .ok_or("Checkbox has no on appearance")?;
                let chosen = match value.as_str() {
                    "true" | "1" | "on" => on,
                    "false" | "0" | "off" => b"Off".to_vec(),
                    _ => return Err(format!("Checkbox {name} requires true or false")),
                };
                if state.as_ref().is_some_and(|s| s != &chosen) {
                    return Err("Checkbox widgets have inconsistent states".into());
                }
                state = Some(chosen);
            }
            let state = state.ok_or("Checkbox has no widgets")?;
            for widget in widgets {
                doc.get_dictionary_mut(widget)
                    .map_err(err)?
                    .set("AS", Object::Name(state.clone()));
            }
            doc.get_dictionary_mut(id)
                .map_err(err)?
                .set("V", Object::Name(state));
            found.insert(name);
            continue;
        }
        if kind.as_deref() != Some(b"Tx") {
            return Err(format!(
                "Field {name} is not a supported text or checkbox field"
            ));
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
fn images(inputs: &[PathBuf], options: &Options) -> Result<Document> {
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
        let margin = number(options, "margin", 0.)?;
        let (mut page_w, mut page_h) = match options
            .get("page_size")
            .map(String::as_str)
            .unwrap_or("fit")
        {
            "fit" => (0., 0.),
            "a4" => (595., 842.),
            "letter" => (612., 792.),
            _ => return Err("page_size must be fit, a4 or letter".into()),
        };
        let landscape = match options
            .get("orientation")
            .map(String::as_str)
            .unwrap_or("portrait")
        {
            "portrait" => false,
            "landscape" => true,
            _ => return Err("orientation must be portrait or landscape".into()),
        };
        if margin < 0. {
            return Err("margin must be nonnegative".into());
        }
        if landscape {
            std::mem::swap(&mut page_w, &mut page_h);
        }
        let (pw, ph, x, y) = if page_w == 0. {
            let scale = (595. / w as f32).min(842. / h as f32);
            let (pw, ph) = (w as f32 * scale, h as f32 * scale);
            page_w = pw + 2. * margin;
            page_h = ph + 2. * margin;
            (pw, ph, margin, margin)
        } else {
            if 2. * margin >= page_w.min(page_h) {
                return Err("margin leaves no room for image".into());
            }
            let scale = ((page_w - 2. * margin) / w as f32).min((page_h - 2. * margin) / h as f32);
            let (pw, ph) = (w as f32 * scale, h as f32 * scale);
            (pw, ph, (page_w - pw) / 2., (page_h - ph) / 2.)
        };
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("q {pw} 0 0 {ph} {x} {y} cm /Image Do Q").into_bytes(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),page_w.into(),page_h.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Image"=>image}},"Contents"=>content});
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
        overlay(
            &mut d,
            id,
            "Visible signature",
            20.,
            30.,
            14.,
            false,
            0.,
            1.,
        )
        .unwrap();
        let bytes = d.get_page_content(id);
        assert!(String::from_utf8_lossy(&bytes).contains("Do"));
        assert!(
            d.objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .any(|s| String::from_utf8_lossy(&s.content).contains("Visible signature"))
        );
        assert!(overlay(&mut d, id, "中文", 0., 0., 12., false, 0., 1.).is_err());
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
    fn advanced_native_options_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        fixture().save(&input).unwrap();
        let crop = Options::from([
            ("pages".into(), "2".into()),
            ("margin_left".into(), "10".into()),
            ("margin_top".into(), "20".into()),
        ]);
        let (path, _) = run("crop", &[input.clone()], &crop, dir.path()).unwrap();
        let d = load(&path).unwrap();
        let box_ = inherited(&d, d.get_pages()[&2], b"CropBox")
            .unwrap()
            .unwrap();
        assert_eq!(
            box_.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_float().unwrap())
                .collect::<Vec<_>>(),
            vec![10., 0., 600., 780.]
        );
        let opts = Options::from([("rotations".into(), r#"{"1":180,"2":270}"#.into())]);
        let (path, _) = run("rotate", &[input.clone()], &opts, dir.path()).unwrap();
        let d = load(&path).unwrap();
        let pages = d.get_pages();
        assert_eq!(
            inherited(&d, pages[&1], b"Rotate")
                .unwrap()
                .unwrap()
                .as_i64()
                .unwrap(),
            180
        );
        assert_eq!(
            inherited(&d, pages[&2], b"Rotate")
                .unwrap()
                .unwrap()
                .as_i64()
                .unwrap(),
            270
        );
        let opts = Options::from([
            ("text".into(), "TEST".into()),
            ("pages".into(), "2".into()),
            ("rotation".into(), "45".into()),
            ("opacity".into(), "0.3".into()),
        ]);
        let (path, _) = run("watermark", &[input.clone()], &opts, dir.path()).unwrap();
        let d = load(&path).unwrap();
        assert!(
            d.objects
                .values()
                .any(
                    |o| o.as_stream().ok().is_some_and(|s| String::from_utf8_lossy(
                        &s.decompressed_content()
                            .unwrap_or_else(|_| s.content.clone())
                    )
                    .contains("ToolOpacity"))
                )
        );
        let opts = Options::from([
            ("format".into(), "Page {n} of {total}".into()),
            ("start".into(), "2".into()),
        ]);
        let (path, _) = run("page-numbers", &[input.clone()], &opts, dir.path()).unwrap();
        let d = load(&path).unwrap();
        assert!(
            d.objects
                .values()
                .any(
                    |o| o.as_stream().ok().is_some_and(|s| String::from_utf8_lossy(
                        &s.decompressed_content()
                            .unwrap_or_else(|_| s.content.clone())
                    )
                    .contains("Page 2 of 3"))
                )
        );
        let image_path = dir.path().join("signature.png");
        image::RgbaImage::from_pixel(10, 5, image::Rgba([0, 0, 0, 128]))
            .save(&image_path)
            .unwrap();
        let opts = Options::from([
            (
                "image_base64".into(),
                base64::engine::general_purpose::STANDARD
                    .encode(std::fs::read(&image_path).unwrap()),
            ),
            ("pages".into(), "1".into()),
        ]);
        let (path, _) = run("sign", &[input], &opts, dir.path()).unwrap();
        let d = load(&path).unwrap();
        assert!(
            d.objects
                .values()
                .any(|o| o.as_stream().ok().is_some_and(|s| s.dict.has(b"SMask")))
        );
        let opts = Options::from([
            ("page_size".into(), "a4".into()),
            ("orientation".into(), "landscape".into()),
            ("margin".into(), "20".into()),
        ]);
        let (path, _) = run("image-to-pdf", &[image_path], &opts, dir.path()).unwrap();
        let d = load(&path).unwrap();
        assert_eq!(
            page_box(&d, d.get_pages()[&1]).unwrap(),
            [0., 0., 842., 595.]
        );
    }
    #[test]
    fn checkbox_and_discovery_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("form.pdf");
        let mut d = fixture();
        let widget = d.add_object(dictionary!{"FT"=>"Btn","T"=>Object::string_literal("agree"),"AP"=>dictionary!{"N"=>dictionary!{"Off"=>dictionary!{},"Yes"=>dictionary!{}}},"AS"=>"Off"});
        let form = d.add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)]});
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root).unwrap().set("AcroForm", form);
        d.save(&input).unwrap();
        let (out, mime) = run(
            "forms",
            &[input.clone()],
            &Options::from([("action".into(), "discover".into())]),
            dir.path(),
        )
        .unwrap();
        assert_eq!(mime, "application/json");
        let fields: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
        assert_eq!(fields[0]["name"], "agree");
        let (out, _) = run(
            "forms",
            &[input],
            &Options::from([("field.agree".into(), "true".into())]),
            dir.path(),
        )
        .unwrap();
        let d = load(&out).unwrap();
        assert_eq!(
            d.get_dictionary(widget)
                .unwrap()
                .get(b"V")
                .unwrap()
                .as_name()
                .unwrap(),
            b"Yes"
        );
    }
    #[test]
    fn numeric_validation() {
        assert!(number(&HashMap::from([("x".into(), "NaN".into())]), "x", 0.).is_err());
    }
}
