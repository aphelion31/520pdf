//! Pure-Rust resume and LaTeX resume PDF generator using `pdf-writer`.
//! Generates A4 PDFs directly without executing external TeX or browser engines.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{ImageEncoder, codecs::jpeg::JpegEncoder, imageops::FilterType};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const MAX_INPUT: u64 = 100 * 1024;
const MAX_PHOTO_BYTES: usize = 2 * 1024 * 1024;
const A4_W: f32 = 595.28;
const A4_H: f32 = 841.89;

#[derive(Deserialize)]
#[serde(untagged)]
enum ExperienceInput {
    Text(String),
    Items(Vec<ExperienceItem>),
}

impl Default for ExperienceInput {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperienceItem {
    #[serde(default)]
    company: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    location: String,
    #[serde(default, alias = "companyDescription")]
    company_description: String,
    #[serde(default, alias = "startDate")]
    start_date: String,
    #[serde(default, alias = "endDate")]
    end_date: String,
    #[serde(default, alias = "isCurrent")]
    is_current: bool,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EducationInput {
    Text(String),
    Items(Vec<EducationItem>),
}

impl Default for EducationInput {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EducationItem {
    #[serde(default)]
    institution: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    degree: String,
    #[serde(default, alias = "fieldOfStudy")]
    field_of_study: String,
    #[serde(default, alias = "startDate")]
    start_date: String,
    #[serde(default, alias = "endDate")]
    end_date: String,
    #[serde(default, alias = "isCurrent")]
    is_current: bool,
    #[serde(default)]
    gpa: String,
    #[serde(default, alias = "maxGpa")]
    max_gpa: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct OrganizationItem {
    #[serde(default)]
    organization: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    location: String,
    #[serde(default, alias = "organizationDescription")]
    organization_description: String,
    #[serde(default, alias = "startDate")]
    start_date: String,
    #[serde(default, alias = "endDate")]
    end_date: String,
    #[serde(default, alias = "isCurrent")]
    is_current: bool,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct OtherItem {
    #[serde(default)]
    category: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SectionTitles {
    #[serde(default)]
    experience: String,
    #[serde(default)]
    education: String,
    #[serde(default)]
    organizations: String,
    #[serde(default)]
    other: String,
}

fn default_font_size() -> f32 {
    10.0
}

fn default_line_spacing() -> f32 {
    1.28
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeStyle {
    #[serde(default)]
    font: String,
    #[serde(default = "default_font_size", alias = "fontSize")]
    font_size: f32,
    #[serde(default)]
    margin: String,
    #[serde(default = "default_line_spacing", alias = "lineSpacing")]
    line_spacing: f32,
    #[serde(default, alias = "headerStyle")]
    header_style: String,
    #[serde(default)]
    alignment: String,
    #[serde(default, alias = "educationFirst")]
    education_first: bool,
    #[serde(default, alias = "accentColor")]
    accent_color: String,
    #[serde(default)]
    language: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SkillEntry {
    Name(String),
    Item {
        name: String,
        #[serde(default)]
        level: String,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SkillsInput {
    Text(String),
    Items(Vec<SkillEntry>),
}

impl Default for SkillsInput {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PersonalInfo {
    #[serde(default, alias = "fullName")]
    full_name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    website: String,
    #[serde(default)]
    linkedin: String,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    #[serde(default, alias = "documentTitle")]
    document_title: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    website: String,
    #[serde(default)]
    linkedin: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    experience: ExperienceInput,
    #[serde(default)]
    education: EducationInput,
    #[serde(default)]
    organizations: Vec<OrganizationItem>,
    #[serde(default)]
    other: Vec<OtherItem>,
    #[serde(default)]
    skills: SkillsInput,
    #[serde(default)]
    template: String,
    #[serde(default, alias = "personalInfo")]
    personal_info: Option<PersonalInfo>,
    #[serde(default, alias = "sectionTitles")]
    section_titles: Option<SectionTitles>,
    #[serde(default)]
    style: Option<ResumeStyle>,
}

impl Resume {
    fn display_name(&self) -> &str {
        if !self.name.trim().is_empty() {
            self.name.trim()
        } else if let Some(info) = &self.personal_info {
            info.full_name.trim()
        } else {
            ""
        }
    }

    fn email(&self) -> &str {
        if !self.email.trim().is_empty() {
            self.email.trim()
        } else if let Some(info) = &self.personal_info {
            info.email.trim()
        } else {
            ""
        }
    }

    fn phone(&self) -> &str {
        if !self.phone.trim().is_empty() {
            self.phone.trim()
        } else if let Some(info) = &self.personal_info {
            info.phone.trim()
        } else {
            ""
        }
    }

    fn location(&self) -> &str {
        if !self.location.trim().is_empty() {
            self.location.trim()
        } else if !self.address.trim().is_empty() {
            self.address.trim()
        } else if let Some(info) = &self.personal_info {
            info.address.trim()
        } else {
            ""
        }
    }

    fn summary(&self) -> &str {
        if !self.summary.trim().is_empty() {
            self.summary.trim()
        } else if let Some(info) = &self.personal_info {
            info.summary.trim()
        } else {
            ""
        }
    }

    fn role_title(&self) -> &str {
        if !self.title.trim().is_empty() {
            self.title.trim()
        } else if let Some(info) = &self.personal_info {
            info.title.trim()
        } else {
            ""
        }
    }

    fn linkedin_url(&self) -> &str {
        if !self.linkedin.trim().is_empty() {
            self.linkedin.trim()
        } else if let Some(info) = &self.personal_info {
            info.linkedin.trim()
        } else {
            ""
        }
    }

    fn portfolio_url(&self) -> &str {
        if !self.website.trim().is_empty() {
            self.website.trim()
        } else if let Some(info) = &self.personal_info {
            info.website.trim()
        } else {
            ""
        }
    }

    #[allow(dead_code)]
    fn website(&self) -> &str {
        let p = self.portfolio_url();
        if !p.is_empty() {
            p
        } else {
            self.linkedin_url()
        }
    }
}

fn parse(bytes: &[u8]) -> Result<Resume, String> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err("Resume exceeds the 100 KB limit".into());
    }
    let resume: Resume = serde_json::from_slice(bytes).map_err(|_| "Invalid resume JSON")?;
    let name = resume.display_name();
    if name.is_empty() || name.len() > 200 {
        return Err("Name is required and must be at most 200 bytes".into());
    }
    for (field_name, value, limit) in [
        ("email", resume.email(), 320),
        ("phone", resume.phone(), 100),
        ("location", resume.location(), 200),
        ("summary", resume.summary(), 10_000),
    ] {
        if value.len() > limit {
            return Err(format!("{field_name} exceeds its {limit} byte limit"));
        }
    }
    if let ExperienceInput::Text(ref t) = resume.experience
        && t.len() > 40_000
    {
        return Err("experience exceeds its 40000 byte limit".into());
    }
    if let EducationInput::Text(ref t) = resume.education
        && t.len() > 20_000
    {
        return Err("education exceeds its 20000 byte limit".into());
    }
    if let SkillsInput::Text(ref t) = resume.skills
        && t.len() > 10_000
    {
        return Err("skills exceeds its 10000 byte limit".into());
    }
    Ok(resume)
}

#[allow(dead_code)]
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[allow(dead_code)]
fn html(resume: &Resume) -> String {
    let contact = [resume.email(), resume.phone()]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| escape(s.trim()))
        .collect::<Vec<_>>()
        .join(" <span class=\"separator\"> / </span> ");
    let exp_text = match &resume.experience {
        ExperienceInput::Text(s) => s.clone(),
        ExperienceInput::Items(items) => items
            .iter()
            .map(|e| format!("{} — {}", e.position, e.company))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let edu_text = match &resume.education {
        EducationInput::Text(s) => s.clone(),
        EducationInput::Items(items) => items
            .iter()
            .map(|e| format!("{} — {}", e.degree, e.institution))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let skills_text = match &resume.skills {
        SkillsInput::Text(s) => s.clone(),
        SkillsInput::Items(items) => items
            .iter()
            .map(|s| match s {
                SkillEntry::Name(n) => n.clone(),
                SkillEntry::Item { name, .. } => name.clone(),
            })
            .collect::<Vec<_>>()
            .join(", "),
    };
    let sections = [
        ("Profile", resume.summary()),
        ("Experience", exp_text.as_str()),
        ("Education", edu_text.as_str()),
        ("Skills", skills_text.as_str()),
    ]
    .into_iter()
    .filter(|(_, text)| !text.trim().is_empty())
    .map(|(title, text)| {
        format!(
            "<section><h2>{title}</h2><p>{}</p></section>",
            escape(text.trim())
        )
    })
    .collect::<String>();
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
@page {{ size: A4; margin: 22mm 23mm; }}
body {{ color: #222b30; font: 10.5pt/1.55 sans-serif; }}
</style></head><body><header><h1>{}</h1><div class="contact">{contact}</div></header>{sections}</body></html>"#,
        escape(resume.display_name())
    )
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FontFamily {
    #[default]
    Helvetica,
    Times,
    Courier,
}

impl FontFamily {
    fn base_fonts(self) -> [(&'static [u8], Ref); 4] {
        let names = match self {
            Self::Helvetica => [
                b"Helvetica".as_slice(),
                b"Helvetica-Bold".as_slice(),
                b"Helvetica-Oblique".as_slice(),
                b"Helvetica-BoldOblique".as_slice(),
            ],
            Self::Times => [
                b"Times-Roman".as_slice(),
                b"Times-Bold".as_slice(),
                b"Times-Italic".as_slice(),
                b"Times-BoldItalic".as_slice(),
            ],
            Self::Courier => [
                b"Courier".as_slice(),
                b"Courier-Bold".as_slice(),
                b"Courier-Oblique".as_slice(),
                b"Courier-BoldOblique".as_slice(),
            ],
        };
        [
            (names[0], Ref::new(4)),
            (names[1], Ref::new(5)),
            (names[2], Ref::new(6)),
            (names[3], Ref::new(7)),
        ]
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FontKind {
    Regular,
    Bold,
    Italic,
    #[allow(dead_code)]
    BoldItalic,
}

impl FontKind {
    fn name(self) -> Name<'static> {
        match self {
            Self::Regular => Name(b"F1"),
            Self::Bold => Name(b"F2"),
            Self::Italic => Name(b"F3"),
            Self::BoldItalic => Name(b"F4"),
        }
    }

    fn avg_width_factor(self) -> f32 {
        match self {
            Self::Regular | Self::Italic => 0.51,
            Self::Bold | Self::BoldItalic => 0.56,
        }
    }
}

fn to_win_ansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            '\u{2022}' => 0x95, // bullet
            '\u{2013}' => 0x96, // en-dash
            '\u{2014}' => 0x97, // em-dash
            '\u{2018}' | '\u{2019}' => b'\'',
            '\u{201C}' | '\u{201D}' => b'"',
            '\u{2026}' => 0x85, // ellipsis
            '\u{00B7}' => 0xB7, // middle dot
            '\u{2212}' => b'-',
            c if (' '..='~').contains(&c) => c as u8,
            c if (0xA0..=0xFF).contains(&(c as u32)) => c as u8,
            '\t' => b' ',
            _ => b'?',
        })
        .collect()
}

fn approx_width_family(text: &str, size: f32, font: FontKind, family: FontFamily) -> f32 {
    if family == FontFamily::Courier {
        return text.chars().count() as f32 * 0.60 * size;
    }
    let mut units = 0.0f32;
    for ch in text.chars() {
        units += match ch {
            ' ' | 'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '|' => 0.30,
            'f' | 'j' | 'r' | 't' | '-' | '/' | '(' | ')' => 0.36,
            'm' | 'w' | 'M' | 'W' => 0.84,
            c if c.is_ascii_uppercase() => 0.68,
            _ => font.avg_width_factor(),
        };
    }
    let scale = match family {
        FontFamily::Times | FontFamily::Courier => 1.0,
        FontFamily::Helvetica => 1.04,
    };
    units * size * scale
}

fn wrap_text_with_ends(
    text: &str,
    max_width: f32,
    size: f32,
    font: FontKind,
    family: FontFamily,
) -> Vec<(String, bool)> {
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let trimmed = raw_line.trim_end();
        if trimmed.is_empty() {
            lines.push((String::new(), true));
            continue;
        }
        let mut current = String::new();
        for word in trimmed.split_whitespace() {
            if current.is_empty() {
                if approx_width_family(word, size, font, family) <= max_width {
                    current.push_str(word);
                } else {
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        chunk.push(ch);
                        if approx_width_family(&chunk, size, font, family) >= max_width {
                            lines.push((std::mem::take(&mut chunk), false));
                        }
                    }
                    current = chunk;
                }
            } else {
                let candidate = format!("{current} {word}");
                if approx_width_family(&candidate, size, font, family) <= max_width {
                    current = candidate;
                } else {
                    lines.push((std::mem::take(&mut current), false));
                    if approx_width_family(word, size, font, family) <= max_width {
                        current.push_str(word);
                    } else {
                        let mut chunk = String::new();
                        for ch in word.chars() {
                            chunk.push(ch);
                            if approx_width_family(&chunk, size, font, family) >= max_width {
                                lines.push((std::mem::take(&mut chunk), false));
                            }
                        }
                        current = chunk;
                    }
                }
            }
        }
        if !current.is_empty() {
            lines.push((current, true));
        }
    }
    if lines.is_empty() {
        lines.push((String::new(), true));
    }
    lines
}

pub(crate) fn wrap_text(text: &str, max_width: f32, size: f32, font: FontKind) -> Vec<String> {
    wrap_text_with_ends(text, max_width, size, font, FontFamily::Helvetica)
        .into_iter()
        .map(|(line, _)| line)
        .collect()
}

pub(crate) struct PdfLayout {
    pages: Vec<Content>,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) margin_x: f32,
    pub(crate) margin_y: f32,
    pub(crate) y: f32,
    font_family: FontFamily,
    first_page_image: Option<(Vec<u8>, u32, u32)>,
}

impl PdfLayout {
    pub(crate) fn a4(margin_x: f32, margin_y: f32) -> Self {
        Self::custom(A4_W, A4_H, margin_x, margin_y)
    }

    pub(crate) fn custom(width: f32, height: f32, margin_x: f32, margin_y: f32) -> Self {
        Self {
            pages: vec![Content::new()],
            width,
            height,
            margin_x,
            margin_y,
            y: height - margin_y,
            font_family: FontFamily::Helvetica,
            first_page_image: None,
        }
    }

    fn set_font_family(&mut self, family: FontFamily) {
        self.font_family = family;
    }

    fn content(&mut self) -> &mut Content {
        self.pages.last_mut().expect("at least one page")
    }

    pub(crate) fn avail(&self, indent: f32) -> f32 {
        (self.width - self.margin_x * 2.0 - indent).max(80.0)
    }

    pub(crate) fn new_page(&mut self) {
        self.pages.push(Content::new());
        self.y = self.height - self.margin_y;
    }

    pub(crate) fn ensure_space(&mut self, needed: f32) {
        if self.y - needed < self.margin_y {
            self.new_page();
        }
    }

    pub(crate) fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: (f32, f32, f32)) {
        let c = self.content();
        c.save_state();
        c.set_fill_rgb(rgb.0, rgb.1, rgb.2);
        c.rect(x, y, w, h);
        c.fill_nonzero();
        c.restore_state();
    }

    pub(crate) fn stroke_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgb: (f32, f32, f32),
        lw: f32,
    ) {
        let c = self.content();
        c.save_state();
        c.set_stroke_rgb(rgb.0, rgb.1, rgb.2);
        c.set_line_width(lw);
        c.rect(x, y, w, h);
        c.stroke();
        c.restore_state();
    }

    fn draw_text_at_spaced(
        &mut self,
        text: &str,
        pos: (f32, f32),
        font: FontKind,
        size: f32,
        color: (f32, f32, f32),
        word_spacing: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let (x, y) = pos;
        let c = self.content();
        c.set_fill_rgb(color.0, color.1, color.2);
        c.begin_text();
        c.set_font(font.name(), size);
        if word_spacing > 0.0 {
            c.set_word_spacing(word_spacing);
        }
        c.next_line(x, y);
        let bytes = to_win_ansi(text);
        c.show(Str(&bytes));
        if word_spacing > 0.0 {
            c.set_word_spacing(0.0);
        }
        c.end_text();
    }

    fn draw_inline_pair(
        &mut self,
        pos: (f32, f32),
        first: (&str, FontKind, f32, (f32, f32, f32)),
        second: (&str, FontKind, f32, (f32, f32, f32), f32),
    ) {
        let (x, y) = pos;
        let (t1, f1, s1, c1) = first;
        let (t2, f2, s2, c2, ws2) = second;
        if t1.is_empty() && t2.is_empty() {
            return;
        }
        let c = self.content();
        c.begin_text();
        c.next_line(x, y);
        if !t1.is_empty() {
            c.set_fill_rgb(c1.0, c1.1, c1.2);
            c.set_font(f1.name(), s1);
            let b1 = to_win_ansi(t1);
            c.show(Str(&b1));
        }
        if !t2.is_empty() {
            c.set_fill_rgb(c2.0, c2.1, c2.2);
            c.set_font(f2.name(), s2);
            if ws2 > 0.0 {
                c.set_word_spacing(ws2);
            }
            let b2 = to_win_ansi(t2);
            c.show(Str(&b2));
            if ws2 > 0.0 {
                c.set_word_spacing(0.0);
            }
        }
        c.end_text();
    }

    pub(crate) fn draw_text_at(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font: FontKind,
        size: f32,
        color: (f32, f32, f32),
    ) {
        self.draw_text_at_spaced(text, (x, y), font, size, color, 0.0);
    }

    fn draw_first_page_photo(
        &mut self,
        jpeg: Vec<u8>,
        size_px: (u32, u32),
        origin: (f32, f32),
        draw_size: (f32, f32),
    ) {
        self.first_page_image = Some((jpeg, size_px.0, size_px.1));
        let c = &mut self.pages[0];
        c.save_state();
        c.transform([draw_size.0, 0.0, 0.0, draw_size.1, origin.0, origin.1]);
        c.x_object(Name(b"Im1"));
        c.restore_state();
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_wrapped_box(
        &mut self,
        text: &str,
        font: FontKind,
        size: f32,
        x_start: f32,
        max_w: f32,
        color: (f32, f32, f32),
        leading_mult: f32,
        justify: bool,
    ) {
        let lines = wrap_text_with_ends(text, max_w, size, font, self.font_family);
        let leading = size * leading_mult;
        for (line, is_end) in lines {
            self.ensure_space(leading);
            let y = self.y - size;
            let extra = if justify && !is_end {
                let spaces = line.bytes().filter(|&b| b == b' ').count();
                if spaces > 0 {
                    let w = approx_width_family(&line, size, font, self.font_family);
                    ((max_w - w) / spaces as f32).clamp(0.0, size * 0.35)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            self.draw_text_at_spaced(&line, (x_start, y), font, size, color, extra);
            self.y -= leading;
        }
    }

    pub(crate) fn draw_wrapped(
        &mut self,
        text: &str,
        font: FontKind,
        size: f32,
        indent: f32,
        color: (f32, f32, f32),
        leading_mult: f32,
    ) {
        let x_start = self.margin_x + indent;
        let max_w = self.avail(indent);
        self.draw_wrapped_box(text, font, size, x_start, max_w, color, leading_mult, false);
    }

    pub(crate) fn draw_centered(
        &mut self,
        text: &str,
        font: FontKind,
        size: f32,
        color: (f32, f32, f32),
    ) {
        let x_left = self.margin_x;
        let max_w = self.avail(0.0);
        let lines = wrap_text_with_ends(text, max_w, size, font, self.font_family);
        let leading = size * 1.35;
        for (line, _) in lines {
            self.ensure_space(leading);
            let w = approx_width_family(&line, size, font, self.font_family);
            let x = (x_left + (max_w - w) * 0.5).max(x_left);
            let y = self.y - size;
            self.draw_text_at(&line, x, y, font, size, color);
            self.y -= leading;
        }
    }

    fn draw_left_right(
        &mut self,
        left: &str,
        right: &str,
        left_font: FontKind,
        size: f32,
        left_color: (f32, f32, f32),
        right_color: (f32, f32, f32),
    ) {
        let leading = size * 1.45;
        self.ensure_space(leading);
        let y = self.y - size;
        self.draw_text_at(left, self.margin_x, y, left_font, size, left_color);
        if !right.trim().is_empty() {
            let rw = approx_width_family(right, size * 0.92, FontKind::Regular, self.font_family);
            let rx = (self.width - self.margin_x - rw).max(self.margin_x + 120.0);
            self.draw_text_at(right, rx, y, FontKind::Regular, size * 0.92, right_color);
        }
        self.y -= leading;
    }

    pub(crate) fn draw_rule(&mut self, color: (f32, f32, f32), thickness: f32) {
        self.ensure_space(8.0);
        let y = self.y - 3.0;
        let x1 = self.margin_x;
        let x2 = self.width - self.margin_x;
        let c = self.content();
        c.save_state();
        c.set_stroke_rgb(color.0, color.1, color.2);
        c.set_line_width(thickness);
        c.move_to(x1, y);
        c.line_to(x2, y);
        c.stroke();
        c.restore_state();
        self.y -= 8.0;
    }

    pub(crate) fn draw_section_heading(&mut self, title: &str, accent: (f32, f32, f32)) {
        self.y -= 6.0;
        self.ensure_space(28.0);
        let upper = title.trim().to_uppercase();
        let y = self.y - 10.5;
        self.draw_text_at(&upper, self.margin_x, y, FontKind::Bold, 10.5, accent);
        self.y -= 15.0;
        self.draw_rule(accent, 0.75);
        self.y -= 2.0;
    }

    fn draw_ats_section_heading(
        &mut self,
        title: &str,
        heading_size: f32,
        text_color: (f32, f32, f32),
        rule_color: (f32, f32, f32),
    ) {
        self.y -= 5.0;
        self.ensure_space(heading_size + 18.0);
        let y = self.y - heading_size;
        self.draw_text_at(
            title.trim(),
            self.margin_x,
            y,
            FontKind::Bold,
            heading_size,
            text_color,
        );
        self.y -= heading_size + 2.5;
        let rule_y = self.y;
        let x1 = self.margin_x;
        let x2 = self.width - self.margin_x;
        let c = self.content();
        c.save_state();
        c.set_stroke_rgb(rule_color.0, rule_color.1, rule_color.2);
        c.set_line_width(0.8);
        c.move_to(x1, rule_y);
        c.line_to(x2, rule_y);
        c.stroke();
        c.restore_state();
        self.y -= 4.5;
    }

    fn draw_entry_header(
        &mut self,
        bold_left: &str,
        location: &str,
        right_date: &str,
        font_spec: (f32, f32),
        ink: (f32, f32, f32),
        muted: (f32, f32, f32),
    ) {
        let (size, leading_mult) = font_spec;
        let right_trimmed = right_date.trim();
        let date_size = size * 0.94;
        let date_w = if right_trimmed.is_empty() {
            0.0
        } else {
            approx_width_family(
                right_trimmed,
                date_size,
                FontKind::Regular,
                self.font_family,
            ) + 12.0
        };
        let left_avail = (self.avail(0.0) - date_w).max(120.0);
        let leading = size * leading_mult;
        let bold_lines = wrap_text_with_ends(
            bold_left.trim(),
            left_avail,
            size,
            FontKind::Bold,
            self.font_family,
        );
        let loc_trimmed = location.trim();
        let loc_suffix = if loc_trimmed.is_empty() {
            String::new()
        } else {
            format!(" - {loc_trimmed}")
        };
        let line_count = bold_lines.len().max(1);
        self.ensure_space(leading * line_count as f32 + 2.0);
        let first_baseline = self.y - size;
        if !right_trimmed.is_empty() {
            let rw = approx_width_family(
                right_trimmed,
                date_size,
                FontKind::Regular,
                self.font_family,
            );
            let rx = (self.width - self.margin_x - rw).max(self.margin_x + 120.0);
            self.draw_text_at(
                right_trimmed,
                rx,
                first_baseline,
                FontKind::Regular,
                date_size,
                ink,
            );
        }

        let last_idx = bold_lines.len().saturating_sub(1);
        for (idx, (line, _)) in bold_lines.iter().enumerate() {
            if idx > 0 {
                self.y -= leading;
            }
            let y = self.y - size;
            if idx < last_idx {
                self.draw_text_at(line, self.margin_x, y, FontKind::Bold, size, ink);
                continue;
            }

            if !loc_suffix.is_empty() {
                let line_w = approx_width_family(line, size, FontKind::Bold, self.font_family);
                let loc_w =
                    approx_width_family(&loc_suffix, size, FontKind::Regular, self.font_family);
                if line_w + loc_w <= left_avail {
                    self.draw_inline_pair(
                        (self.margin_x, y),
                        (line, FontKind::Bold, size, ink),
                        (&loc_suffix, FontKind::Regular, size, muted, 0.0),
                    );
                    self.y -= leading;
                } else {
                    self.draw_text_at(line, self.margin_x, y, FontKind::Bold, size, ink);
                    self.y -= leading;
                    self.draw_wrapped_box(
                        loc_trimmed,
                        FontKind::Regular,
                        size * 0.94,
                        self.margin_x,
                        left_avail,
                        muted,
                        leading_mult,
                        false,
                    );
                }
            } else {
                self.draw_text_at(line, self.margin_x, y, FontKind::Bold, size, ink);
                self.y -= leading;
            }
        }
    }

    fn draw_hanging_bullet(
        &mut self,
        text: &str,
        size: f32,
        color: (f32, f32, f32),
        leading_mult: f32,
        justify: bool,
    ) {
        let indent = 14.0;
        let bullet_x = self.margin_x + 5.0;
        let text_x = self.margin_x + indent;
        let max_w = self.avail(indent);
        let lines = wrap_text_with_ends(text, max_w, size, FontKind::Regular, self.font_family);
        let leading = size * leading_mult;
        for (idx, (line, is_end)) in lines.into_iter().enumerate() {
            self.ensure_space(leading);
            let y = self.y - size;
            if idx == 0 {
                self.draw_text_at("\u{2022}", bullet_x, y, FontKind::Regular, size, color);
            }
            let extra = if justify && !is_end {
                let spaces = line.bytes().filter(|&b| b == b' ').count();
                if spaces > 0 {
                    let w = approx_width_family(&line, size, FontKind::Regular, self.font_family);
                    ((max_w - w) / spaces as f32).clamp(0.0, size * 0.35)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            self.draw_text_at_spaced(&line, (text_x, y), FontKind::Regular, size, color, extra);
            self.y -= leading;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_labeled_bullet(
        &mut self,
        category: &str,
        year: &str,
        detail: &str,
        size: f32,
        ink: (f32, f32, f32),
        leading_mult: f32,
        justify: bool,
    ) {
        let indent = 14.0;
        let bullet_x = self.margin_x + 5.0;
        let text_x = self.margin_x + indent;
        let max_w = self.avail(indent);
        let leading = size * leading_mult;

        let cat = category.trim();
        let yr = year.trim();
        let label = match (cat.is_empty(), yr.is_empty()) {
            (false, false) => format!("{cat} ({yr}):"),
            (false, true) => format!("{cat}:"),
            (true, false) => format!("({yr}):"),
            (true, true) => String::new(),
        };

        if label.is_empty() {
            for para in detail.lines().map(str::trim).filter(|l| !l.is_empty()) {
                self.draw_hanging_bullet(para, size, ink, leading_mult, justify);
            }
            return;
        }

        let clean_detail = detail
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let label_spaced = format!("{label} ");
        let label_w = approx_width_family(&label_spaced, size, FontKind::Bold, self.font_family);
        let first_avail = max_w - label_w;
        self.ensure_space(leading);
        let y_first = self.y - size;
        self.draw_text_at("\u{2022}", bullet_x, y_first, FontKind::Regular, size, ink);

        if clean_detail.is_empty() {
            self.draw_text_at(&label, text_x, y_first, FontKind::Bold, size, ink);
            self.y -= leading;
            return;
        }

        let words: Vec<&str> = clean_detail.split_whitespace().collect();
        let mut split_idx = 0usize;
        let mut first_line = String::new();
        if first_avail >= 40.0 {
            for (idx, word) in words.iter().enumerate() {
                let candidate = if first_line.is_empty() {
                    (*word).to_string()
                } else {
                    format!("{first_line} {word}")
                };
                if approx_width_family(&candidate, size, FontKind::Regular, self.font_family)
                    <= first_avail
                {
                    first_line = candidate;
                    split_idx = idx + 1;
                } else {
                    break;
                }
            }
        }

        if !first_line.is_empty() {
            let has_rest = split_idx < words.len();
            let extra = if justify && has_rest {
                let spaces = first_line.bytes().filter(|&b| b == b' ').count();
                if spaces > 0 {
                    let w =
                        approx_width_family(&first_line, size, FontKind::Regular, self.font_family);
                    ((first_avail - w) / spaces as f32).clamp(0.0, size * 0.35)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            self.draw_inline_pair(
                (text_x, y_first),
                (&label_spaced, FontKind::Bold, size, ink),
                (&first_line, FontKind::Regular, size, ink, extra),
            );
        } else {
            self.draw_text_at(&label, text_x, y_first, FontKind::Bold, size, ink);
        }
        self.y -= leading;

        if split_idx < words.len() {
            let rest = words[split_idx..].join(" ");
            self.draw_wrapped_box(
                &rest,
                FontKind::Regular,
                size,
                text_x,
                max_w,
                ink,
                leading_mult,
                justify,
            );
        }
    }

    pub(crate) fn finish_pdf(self, title: &str) -> Vec<u8> {
        let mut pdf = Pdf::new();
        let catalog_id = Ref::new(1);
        let pages_id = Ref::new(2);
        let info_id = Ref::new(3);
        let font_refs = self.font_family.base_fonts();

        pdf.catalog(catalog_id).pages(pages_id);
        {
            let mut info = pdf.document_info(info_id);
            info.title(TextStr(title));
            info.producer(TextStr("520pdf"));
            info.creator(TextStr("520pdf"));
        }
        for (base, id) in font_refs {
            let mut font = pdf.type1_font(id);
            font.base_font(Name(base));
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }

        let mut next_id = 8i32;
        let img_id = if let Some((ref jpeg_bytes, w_px, h_px)) = self.first_page_image {
            let id = Ref::new(next_id);
            next_id += 1;
            let mut img = pdf.image_xobject(id, jpeg_bytes);
            img.filter(pdf_writer::Filter::DctDecode);
            img.width(w_px as i32);
            img.height(h_px as i32);
            img.color_space().device_rgb();
            img.bits_per_component(8);
            img.finish();
            Some(id)
        } else {
            None
        };

        let mut page_ids = Vec::with_capacity(self.pages.len());
        let page_pairs: Vec<(Ref, Ref, Content)> = self
            .pages
            .into_iter()
            .map(|content| {
                let pid = Ref::new(next_id);
                let cid = Ref::new(next_id + 1);
                next_id += 2;
                page_ids.push(pid);
                (pid, cid, content)
            })
            .collect();

        pdf.pages(pages_id)
            .kids(page_ids)
            .count(page_pairs.len() as i32);
        for (idx, (page_id, content_id, content)) in page_pairs.into_iter().enumerate() {
            let data = content.finish();
            pdf.stream(content_id, &data);
            let mut page = pdf.page(page_id);
            page.parent(pages_id);
            page.media_box(Rect::new(0.0, 0.0, self.width, self.height));
            page.contents(content_id);
            let mut res = page.resources();
            {
                let mut fonts = res.fonts();
                fonts.pair(Name(b"F1"), font_refs[0].1);
                fonts.pair(Name(b"F2"), font_refs[1].1);
                fonts.pair(Name(b"F3"), font_refs[2].1);
                fonts.pair(Name(b"F4"), font_refs[3].1);
            }
            if idx == 0
                && let Some(iid) = img_id
            {
                res.x_objects().pair(Name(b"Im1"), iid);
            }
        }

        pdf.finish()
    }
}

fn decode_portrait_photo(raw: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Empty photo".into());
    }
    let payload = if trimmed.starts_with("data:") {
        let (_, b64) = trimmed
            .split_once(";base64,")
            .ok_or("Photo must be a base64 image")?;
        b64
    } else {
        trimmed
    };
    let clean: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() > MAX_PHOTO_BYTES * 4 / 3 + 16 {
        return Err("Photo exceeds 2 MB limit".into());
    }
    let decoded = BASE64
        .decode(clean.as_bytes())
        .map_err(|_| "Invalid base64 photo data")?;
    if decoded.len() > MAX_PHOTO_BYTES {
        return Err("Photo exceeds 2 MB limit".into());
    }
    let img = image::load_from_memory(&decoded).map_err(|_| "Unsupported portrait image format")?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err("Invalid portrait photo dimensions".into());
    }
    let target_ratio = 0.8f32;
    let src_ratio = w as f32 / h as f32;
    let cropped = if src_ratio > target_ratio {
        let crop_w = ((h as f32 * target_ratio).round() as u32).clamp(1, w);
        let x0 = (w - crop_w) / 2;
        img.crop_imm(x0, 0, crop_w, h)
    } else {
        let crop_h = ((w as f32 / target_ratio).round() as u32).clamp(1, h);
        let y0 = (h - crop_h) / 4;
        img.crop_imm(0, y0, w, crop_h)
    };
    let max_w = 360u32;
    let max_h = 450u32;
    let resized = if cropped.width() > max_w || cropped.height() > max_h {
        cropped.resize_exact(max_w, max_h, FilterType::Triangle)
    } else {
        cropped
    };
    let rgb = resized.to_rgb8();
    let (out_w, out_h) = (rgb.width(), rgb.height());
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 88)
        .write_image(rgb.as_raw(), out_w, out_h, image::ExtendedColorType::Rgb8)
        .map_err(|_| "Cannot encode portrait photo")?;
    Ok((out, out_w, out_h))
}

fn parse_hex_color(raw: &str) -> Option<(f32, f32, f32)> {
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

fn format_date_range_lang(start: &str, end: &str, is_current: bool, present_label: &str) -> String {
    let s = start.trim();
    let e = if is_current {
        present_label
    } else {
        end.trim()
    };
    match (s, e) {
        ("", "") => String::new(),
        (st, "") => format!("{st} - {present_label}"),
        ("", en) => en.to_string(),
        (st, en) => format!("{st} - {en}"),
    }
}

fn render_ats_resume_pdf(
    resume: &Resume,
    template_override: Option<&str>,
    photo: Option<(Vec<u8>, u32, u32)>,
) -> Vec<u8> {
    let style = resume.style.as_ref();
    let lang_id = style
        .map(|s| s.language.trim().eq_ignore_ascii_case("id"))
        .unwrap_or(false);
    let present_label = if lang_id { "Sekarang" } else { "Present" };

    let font_family = match style.map(|s| s.font.trim().to_ascii_lowercase()).as_deref() {
        Some("times" | "times-new-roman" | "serif") => FontFamily::Times,
        Some("courier" | "mono") => FontFamily::Courier,
        _ => FontFamily::Helvetica,
    };

    let body_size = style.map(|s| s.font_size).unwrap_or(10.0).clamp(8.5, 12.5);
    let leading_mult = style
        .map(|s| s.line_spacing)
        .unwrap_or(1.28)
        .clamp(1.1, 1.7);
    let justify = style
        .map(|s| s.alignment.trim().eq_ignore_ascii_case("justify"))
        .unwrap_or(false);
    let education_first = style.map(|s| s.education_first).unwrap_or(false);

    let (mx, my) = match style
        .map(|s| s.margin.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("small" | "compact") => (36.0, 34.0),
        Some("large" | "wide") => (56.0, 52.0),
        _ => (46.0, 44.0),
    };

    let tpl = template_override
        .filter(|s| !s.trim().is_empty())
        .or_else(|| style.map(|s| s.header_style.as_str()))
        .unwrap_or(resume.template.as_str())
        .trim()
        .to_ascii_lowercase();

    let is_modern = tpl == "modern";
    let is_centered = tpl == "centered" || tpl == "professional";

    let ink = (0.09, 0.10, 0.12);
    let muted = (0.34, 0.38, 0.44);
    let default_accent = if is_modern {
        (0.78, 0.16, 0.21)
    } else {
        (0.11, 0.12, 0.15)
    };
    let accent = style
        .and_then(|s| parse_hex_color(&s.accent_color))
        .unwrap_or(default_accent);
    let heading_text_color = if is_modern { accent } else { ink };
    let rule_color = if is_modern {
        accent
    } else {
        (0.18, 0.20, 0.24)
    };

    let mut layout = PdfLayout::a4(mx, my);
    layout.set_font_family(font_family);

    let contact_parts: Vec<&str> = [
        resume.phone(),
        resume.email(),
        resume.linkedin_url(),
        resume.portfolio_url(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();
    let contact_line = contact_parts.join(" | ");
    let name_upper = resume.display_name().to_uppercase();
    let name_size = (body_size * 1.48).clamp(13.5, 19.0);
    let sub_size = (body_size * 0.88).clamp(7.8, 10.5);

    if let Some((jpeg, w_px, h_px)) = photo {
        let photo_w = 64.0;
        let photo_h = 80.0;
        let top_y = layout.y;
        let photo_bottom = top_y - photo_h;
        layout.draw_first_page_photo(jpeg, (w_px, h_px), (mx, photo_bottom), (photo_w, photo_h));

        let text_x = mx + photo_w + 14.0;
        let text_w = layout.avail(photo_w + 14.0);
        layout.draw_wrapped_box(
            &name_upper,
            FontKind::Bold,
            name_size,
            text_x,
            text_w,
            ink,
            1.18,
            false,
        );
        if !resume.role_title().is_empty() {
            layout.draw_wrapped_box(
                resume.role_title(),
                FontKind::Italic,
                body_size,
                text_x,
                text_w,
                if is_modern { accent } else { muted },
                1.22,
                false,
            );
        }
        if !contact_line.is_empty() {
            layout.draw_wrapped_box(
                &contact_line,
                FontKind::Regular,
                sub_size,
                text_x,
                text_w,
                muted,
                1.22,
                false,
            );
        }
        if !resume.location().is_empty() {
            layout.draw_wrapped_box(
                resume.location(),
                FontKind::Regular,
                sub_size,
                text_x,
                text_w,
                muted,
                1.22,
                false,
            );
        }
        if !resume.summary().is_empty() {
            layout.y -= 2.5;
            layout.draw_wrapped_box(
                resume.summary(),
                FontKind::Regular,
                body_size * 0.96,
                text_x,
                text_w,
                ink,
                leading_mult,
                justify,
            );
        }
        layout.y = layout.y.min(photo_bottom) - 6.0;
    } else if is_modern && !is_centered {
        layout.draw_wrapped(&name_upper, FontKind::Bold, name_size, 0.0, ink, 1.18);
        if !resume.role_title().is_empty() {
            layout.draw_wrapped(
                resume.role_title(),
                FontKind::Italic,
                body_size,
                0.0,
                accent,
                1.22,
            );
        }
        if !contact_line.is_empty() {
            layout.draw_wrapped(&contact_line, FontKind::Regular, sub_size, 0.0, muted, 1.24);
        }
        if !resume.location().is_empty() {
            layout.draw_wrapped(
                resume.location(),
                FontKind::Regular,
                sub_size,
                0.0,
                muted,
                1.24,
            );
        }
        layout.y -= 3.0;
        layout.draw_rule(accent, 1.2);
        if !resume.summary().is_empty() {
            layout.draw_wrapped_box(
                resume.summary(),
                FontKind::Regular,
                body_size * 0.96,
                mx,
                layout.avail(0.0),
                ink,
                leading_mult,
                justify,
            );
            layout.y -= 3.0;
        }
    } else {
        layout.draw_centered(&name_upper, FontKind::Bold, name_size, ink);
        if !resume.role_title().is_empty() {
            layout.draw_centered(resume.role_title(), FontKind::Italic, body_size, muted);
        }
        if !contact_line.is_empty() {
            layout.draw_centered(&contact_line, FontKind::Regular, sub_size, muted);
        }
        if !resume.location().is_empty() {
            layout.draw_centered(resume.location(), FontKind::Regular, sub_size, muted);
        }
        if !resume.summary().is_empty() {
            layout.y -= 3.0;
            layout.draw_wrapped_box(
                resume.summary(),
                FontKind::Regular,
                body_size * 0.96,
                mx,
                layout.avail(0.0),
                ink,
                leading_mult,
                justify,
            );
        }
        layout.y -= 4.0;
    }

    let titles = resume.section_titles.as_ref();
    let edu_heading = titles
        .map(|t| t.education.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(if lang_id {
            "Jenjang Pendidikan"
        } else {
            "Education"
        });
    let exp_heading = titles
        .map(|t| t.experience.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(if lang_id {
            "Pengalaman Kerja"
        } else {
            "Work Experience"
        });
    let org_heading = titles
        .map(|t| t.organizations.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(if lang_id {
            "Pengalaman Organisasi"
        } else {
            "Organisational Experience"
        });
    let other_heading = titles
        .map(|t| t.other.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(if lang_id {
            "Keterampilan, Prestasi & Pengalaman Lain"
        } else {
            "Skills, Achievements & Other Experience"
        });

    let heading_size = (body_size * 1.06).clamp(9.5, 13.0);

    let draw_education = |layout: &mut PdfLayout| match &resume.education {
        EducationInput::Text(s) if !s.trim().is_empty() => {
            layout.draw_ats_section_heading(
                edu_heading,
                heading_size,
                heading_text_color,
                rule_color,
            );
            layout.draw_wrapped_box(
                s.trim(),
                FontKind::Regular,
                body_size,
                mx,
                layout.avail(0.0),
                ink,
                leading_mult,
                justify,
            );
        }
        EducationInput::Items(items) if !items.is_empty() => {
            layout.draw_ats_section_heading(
                edu_heading,
                heading_size,
                heading_text_color,
                rule_color,
            );
            for item in items {
                let dates = format_date_range_lang(
                    &item.start_date,
                    &item.end_date,
                    item.is_current,
                    present_label,
                );
                let primary = if !item.institution.trim().is_empty() {
                    item.institution.trim()
                } else {
                    item.degree.trim()
                };
                if !primary.is_empty() || !dates.is_empty() {
                    layout.draw_entry_header(
                        primary,
                        &item.location,
                        &dates,
                        (body_size, leading_mult),
                        ink,
                        muted,
                    );
                }
                let mut deg_parts = Vec::new();
                let deg = item.degree.trim();
                let field = item.field_of_study.trim();
                if !deg.is_empty() && !field.is_empty() {
                    let lower_f = field.to_ascii_lowercase();
                    if lower_f.starts_with("of ") || lower_f.starts_with("in ") {
                        deg_parts.push(format!("{deg} {field}"));
                    } else if lower_f.starts_with("majoring")
                        || deg.ends_with('.')
                        || deg.len() <= 4
                    {
                        deg_parts.push(format!("{deg} — {field}"));
                    } else {
                        deg_parts.push(format!("{deg} in {field}"));
                    }
                } else if !deg.is_empty() && deg != primary {
                    deg_parts.push(deg.to_string());
                } else if !field.is_empty() {
                    deg_parts.push(field.to_string());
                }
                let gpa = item.gpa.trim();
                let max_gpa = item.max_gpa.trim();
                if !gpa.is_empty() {
                    if max_gpa.is_empty() {
                        deg_parts.push(format!("GPA {gpa}"));
                    } else {
                        deg_parts.push(format!("{gpa}/{max_gpa}"));
                    }
                }
                if !deg_parts.is_empty() {
                    let sub_line = deg_parts.join(", ");
                    layout.draw_wrapped_box(
                        &sub_line,
                        FontKind::Italic,
                        body_size * 0.96,
                        mx,
                        layout.avail(0.0),
                        ink,
                        leading_mult,
                        false,
                    );
                }
                for bullet in item
                    .description
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                {
                    let cleaned = bullet.trim_start_matches(['•', '-', '*']).trim();
                    layout.draw_hanging_bullet(
                        cleaned,
                        body_size * 0.96,
                        ink,
                        leading_mult,
                        justify,
                    );
                }
                layout.y -= 3.5;
            }
        }
        _ => {}
    };

    let draw_experience = |layout: &mut PdfLayout| match &resume.experience {
        ExperienceInput::Text(s) if !s.trim().is_empty() => {
            layout.draw_ats_section_heading(
                exp_heading,
                heading_size,
                heading_text_color,
                rule_color,
            );
            layout.draw_wrapped_box(
                s.trim(),
                FontKind::Regular,
                body_size,
                mx,
                layout.avail(0.0),
                ink,
                leading_mult,
                justify,
            );
        }
        ExperienceInput::Items(items) if !items.is_empty() => {
            layout.draw_ats_section_heading(
                exp_heading,
                heading_size,
                heading_text_color,
                rule_color,
            );
            for item in items {
                let dates = format_date_range_lang(
                    &item.start_date,
                    &item.end_date,
                    item.is_current,
                    present_label,
                );
                let primary = if !item.company.trim().is_empty() {
                    item.company.trim()
                } else {
                    item.position.trim()
                };
                if !primary.is_empty() || !dates.is_empty() {
                    layout.draw_entry_header(
                        primary,
                        &item.location,
                        &dates,
                        (body_size, leading_mult),
                        ink,
                        muted,
                    );
                }
                if !item.position.trim().is_empty() && item.position.trim() != primary {
                    layout.draw_wrapped_box(
                        item.position.trim(),
                        FontKind::Italic,
                        body_size * 0.96,
                        mx,
                        layout.avail(0.0),
                        ink,
                        leading_mult,
                        false,
                    );
                }
                if !item.company_description.trim().is_empty() {
                    layout.draw_wrapped_box(
                        item.company_description.trim(),
                        FontKind::Italic,
                        body_size * 0.92,
                        mx,
                        layout.avail(0.0),
                        muted,
                        leading_mult,
                        justify,
                    );
                }
                for bullet in item
                    .description
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                {
                    let cleaned = bullet.trim_start_matches(['•', '-', '*']).trim();
                    layout.draw_hanging_bullet(
                        cleaned,
                        body_size * 0.96,
                        ink,
                        leading_mult,
                        justify,
                    );
                }
                layout.y -= 4.0;
            }
        }
        _ => {}
    };

    if education_first {
        draw_education(&mut layout);
        draw_experience(&mut layout);
    } else {
        draw_experience(&mut layout);
        draw_education(&mut layout);
    }

    if !resume.organizations.is_empty() {
        layout.draw_ats_section_heading(org_heading, heading_size, heading_text_color, rule_color);
        for item in &resume.organizations {
            let dates = format_date_range_lang(
                &item.start_date,
                &item.end_date,
                item.is_current,
                present_label,
            );
            let primary = if !item.organization.trim().is_empty() {
                item.organization.trim()
            } else {
                item.position.trim()
            };
            if !primary.is_empty() || !dates.is_empty() {
                layout.draw_entry_header(
                    primary,
                    &item.location,
                    &dates,
                    (body_size, leading_mult),
                    ink,
                    muted,
                );
            }
            if !item.position.trim().is_empty() && item.position.trim() != primary {
                layout.draw_wrapped_box(
                    item.position.trim(),
                    FontKind::Italic,
                    body_size * 0.96,
                    mx,
                    layout.avail(0.0),
                    ink,
                    leading_mult,
                    false,
                );
            }
            if !item.organization_description.trim().is_empty() {
                layout.draw_wrapped_box(
                    item.organization_description.trim(),
                    FontKind::Italic,
                    body_size * 0.92,
                    mx,
                    layout.avail(0.0),
                    muted,
                    leading_mult,
                    justify,
                );
            }
            for bullet in item
                .description
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
            {
                let cleaned = bullet.trim_start_matches(['•', '-', '*']).trim();
                layout.draw_hanging_bullet(cleaned, body_size * 0.96, ink, leading_mult, justify);
            }
            layout.y -= 4.0;
        }
    }

    let has_other = resume
        .other
        .iter()
        .any(|o| !o.category.trim().is_empty() || !o.description.trim().is_empty());
    if has_other {
        layout.draw_ats_section_heading(
            other_heading,
            heading_size,
            heading_text_color,
            rule_color,
        );
        for item in &resume.other {
            if item.category.trim().is_empty() && item.description.trim().is_empty() {
                continue;
            }
            layout.draw_labeled_bullet(
                &item.category,
                &item.year,
                &item.description,
                body_size * 0.96,
                ink,
                leading_mult,
                justify,
            );
            layout.y -= 2.0;
        }
    } else {
        match &resume.skills {
            SkillsInput::Text(s) if !s.trim().is_empty() => {
                layout.draw_ats_section_heading(
                    other_heading,
                    heading_size,
                    heading_text_color,
                    rule_color,
                );
                layout.draw_wrapped_box(
                    s.trim(),
                    FontKind::Regular,
                    body_size,
                    mx,
                    layout.avail(0.0),
                    ink,
                    leading_mult,
                    justify,
                );
            }
            SkillsInput::Items(items) if !items.is_empty() => {
                let joined = items
                    .iter()
                    .filter_map(|s| match s {
                        SkillEntry::Name(n) if !n.trim().is_empty() => Some(n.trim().to_string()),
                        SkillEntry::Item { name, level } if !name.trim().is_empty() => {
                            if level.trim().is_empty() {
                                Some(name.trim().to_string())
                            } else {
                                Some(format!("{} ({})", name.trim(), level.trim()))
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if !joined.is_empty() {
                    layout.draw_ats_section_heading(
                        other_heading,
                        heading_size,
                        heading_text_color,
                        rule_color,
                    );
                    layout.draw_labeled_bullet(
                        "Skills",
                        "",
                        &joined,
                        body_size * 0.96,
                        ink,
                        leading_mult,
                        justify,
                    );
                }
            }
            _ => {}
        }
    }

    let doc_title = if resume.document_title.trim().is_empty() {
        resume.display_name()
    } else {
        resume.document_title.trim()
    };
    layout.finish_pdf(doc_title)
}

fn template_palette(template: &str) -> ((f32, f32, f32), (f32, f32, f32), bool) {
    match template.trim().to_ascii_lowercase().as_str() {
        "modern" => ((0.78, 0.16, 0.21), (0.14, 0.16, 0.20), false),
        "professional" | "classic" => ((0.06, 0.09, 0.14), (0.12, 0.14, 0.18), true),
        _ => ((0.18, 0.20, 0.24), (0.16, 0.18, 0.21), false),
    }
}

fn render_resume_pdf(
    resume: &Resume,
    template_override: Option<&str>,
    photo: Option<(Vec<u8>, u32, u32)>,
) -> Vec<u8> {
    let tpl = template_override
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(resume.template.as_str());
    let use_ats = resume.style.is_some()
        || !resume.organizations.is_empty()
        || !resume.other.is_empty()
        || resume.section_titles.is_some()
        || photo.is_some()
        || tpl.trim().eq_ignore_ascii_case("ats");
    if use_ats {
        return render_ats_resume_pdf(resume, template_override, photo);
    }

    let (accent, ink, centered) = template_palette(tpl);
    let muted = (0.36, 0.40, 0.45);
    let mut layout = PdfLayout::a4(52.0, 50.0);

    if centered {
        layout.draw_centered(resume.display_name(), FontKind::Bold, 22.0, ink);
    } else {
        layout.draw_wrapped(resume.display_name(), FontKind::Bold, 22.0, 0.0, ink, 1.25);
    }
    if !resume.role_title().is_empty() {
        if centered {
            layout.draw_centered(resume.role_title(), FontKind::Italic, 11.0, accent);
        } else {
            layout.draw_wrapped(
                resume.role_title(),
                FontKind::Italic,
                11.0,
                0.0,
                accent,
                1.3,
            );
        }
    }

    let contact_parts: Vec<&str> = [
        resume.email(),
        resume.phone(),
        resume.location(),
        resume.website(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();
    if !contact_parts.is_empty() {
        let line = contact_parts.join("  \u{00B7}  ");
        if centered {
            layout.draw_centered(&line, FontKind::Regular, 9.5, muted);
        } else {
            layout.draw_wrapped(&line, FontKind::Regular, 9.5, 0.0, muted, 1.35);
        }
    }
    layout.y -= 4.0;
    layout.draw_rule(accent, 1.2);

    if !resume.summary().is_empty() {
        layout.draw_section_heading("Profile", accent);
        layout.draw_wrapped(resume.summary(), FontKind::Regular, 10.0, 0.0, ink, 1.45);
    }

    match &resume.experience {
        ExperienceInput::Text(s) if !s.trim().is_empty() => {
            layout.draw_section_heading("Experience", accent);
            layout.draw_wrapped(s.trim(), FontKind::Regular, 10.0, 0.0, ink, 1.45);
        }
        ExperienceInput::Items(items) if !items.is_empty() => {
            layout.draw_section_heading("Experience", accent);
            for item in items {
                let heading = match (item.position.trim(), item.company.trim()) {
                    ("", c) => c.to_string(),
                    (p, "") => p.to_string(),
                    (p, c) => format!("{p} — {c}"),
                };
                let dates = format_date_range(&item.start_date, &item.end_date);
                if !heading.is_empty() || !dates.is_empty() {
                    layout.draw_left_right(&heading, &dates, FontKind::Bold, 10.5, ink, muted);
                }
                if !item.location.trim().is_empty() {
                    layout.draw_wrapped(
                        item.location.trim(),
                        FontKind::Italic,
                        9.2,
                        0.0,
                        muted,
                        1.3,
                    );
                }
                for bullet in item
                    .description
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                {
                    let cleaned = bullet.trim_start_matches(['•', '-', '*']).trim();
                    layout.draw_wrapped(
                        &format!("\u{2022} {cleaned}"),
                        FontKind::Regular,
                        9.8,
                        10.0,
                        ink,
                        1.4,
                    );
                }
                layout.y -= 4.0;
            }
        }
        _ => {}
    }

    match &resume.education {
        EducationInput::Text(s) if !s.trim().is_empty() => {
            layout.draw_section_heading("Education", accent);
            layout.draw_wrapped(s.trim(), FontKind::Regular, 10.0, 0.0, ink, 1.45);
        }
        EducationInput::Items(items) if !items.is_empty() => {
            layout.draw_section_heading("Education", accent);
            for item in items {
                let degree_line = [item.degree.trim(), item.field_of_study.trim()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" in ");
                let title = if degree_line.is_empty() {
                    item.institution.trim().to_string()
                } else {
                    degree_line
                };
                let dates = format_date_range(&item.start_date, &item.end_date);
                if !title.is_empty() || !dates.is_empty() {
                    layout.draw_left_right(&title, &dates, FontKind::Bold, 10.3, ink, muted);
                }
                if !item.institution.trim().is_empty() && title != item.institution.trim() {
                    layout.draw_wrapped(
                        item.institution.trim(),
                        FontKind::Italic,
                        9.5,
                        0.0,
                        muted,
                        1.3,
                    );
                }
                if !item.description.trim().is_empty() {
                    layout.draw_wrapped(
                        item.description.trim(),
                        FontKind::Regular,
                        9.6,
                        10.0,
                        ink,
                        1.4,
                    );
                }
                layout.y -= 3.0;
            }
        }
        _ => {}
    }

    match &resume.skills {
        SkillsInput::Text(s) if !s.trim().is_empty() => {
            layout.draw_section_heading("Skills", accent);
            layout.draw_wrapped(s.trim(), FontKind::Regular, 10.0, 0.0, ink, 1.45);
        }
        SkillsInput::Items(items) if !items.is_empty() => {
            layout.draw_section_heading("Skills", accent);
            let joined = items
                .iter()
                .filter_map(|s| match s {
                    SkillEntry::Name(n) if !n.trim().is_empty() => Some(n.trim().to_string()),
                    SkillEntry::Item { name, level } if !name.trim().is_empty() => {
                        if level.trim().is_empty() {
                            Some(name.trim().to_string())
                        } else {
                            Some(format!("{} ({})", name.trim(), level.trim()))
                        }
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("  \u{00B7}  ");
            if !joined.is_empty() {
                layout.draw_wrapped(&joined, FontKind::Regular, 10.0, 0.0, ink, 1.45);
            }
        }
        _ => {}
    }

    layout.finish_pdf(resume.display_name())
}

fn format_date_range(start: &str, end: &str) -> String {
    match (start.trim(), end.trim()) {
        ("", "") => String::new(),
        (s, "") => format!("{s} \u{2013} Present"),
        ("", e) => e.to_string(),
        (s, e) => format!("{s} \u{2013} {e}"),
    }
}

fn unwrap_braced_cmd(mut text: String, cmd: &str) -> String {
    let needle = format!("\\{cmd}{{");
    while let Some(pos) = text.find(&needle) {
        let after = pos + needle.len();
        if let Some(end_rel) = find_matching_brace(&text[after..]) {
            let inner = text[after..after + end_rel].to_string();
            text.replace_range(pos..after + end_rel + 1, &inner);
        } else {
            break;
        }
    }
    text
}

fn unwrap_href(mut text: String) -> String {
    while let Some(pos) = text.find("\\href{") {
        let first_start = pos + 6;
        let Some(first_end) = find_matching_brace(&text[first_start..]) else {
            break;
        };
        let second_open = first_start + first_end + 1;
        if !text[second_open..].starts_with('{') {
            break;
        }
        let second_start = second_open + 1;
        let Some(second_end) = find_matching_brace(&text[second_start..]) else {
            break;
        };
        let label = text[second_start..second_start + second_end].to_string();
        text.replace_range(pos..second_start + second_end + 1, &label);
    }
    text
}

fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (idx, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn clean_latex_inline(raw: &str) -> String {
    let mut s = unwrap_href(raw.to_string());
    for cmd in [
        "textbf",
        "textit",
        "emph",
        "url",
        "underline",
        "texttt",
        "textsc",
    ] {
        s = unwrap_braced_cmd(s, cmd);
    }
    for macro_kw in [
        "\\LARGE",
        "\\Large",
        "\\large",
        "\\huge",
        "\\Huge",
        "\\small",
        "\\normalsize",
        "\\bfseries",
        "\\itshape",
    ] {
        s = s.replace(macro_kw, "");
    }
    // Strip optional spacing bracket args like \\[4pt]
    while let Some(pos) = s.find("\\\\[") {
        if let Some(end) = s[pos + 3..].find(']') {
            s.replace_range(pos..pos + 4 + end, " ");
        } else {
            break;
        }
    }
    s = s
        .replace("$\\cdot$", "\u{00B7}")
        .replace("\\cdot", "\u{00B7}")
        .replace("\\%", "%")
        .replace("\\&", "&")
        .replace("\\$", "$")
        .replace("\\_", "_")
        .replace("\\#", "#")
        .replace("\\\\", " ")
        .replace("\\hfill", "  ")
        .replace("---", "\u{2014}")
        .replace("--", "\u{2013}")
        .replace(['{', '}'], "")
        .replace("~~", " ")
        .replace('~', " ");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_section_arg(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let end = find_matching_brace(rest)?;
    Some(clean_latex_inline(&rest[..end]))
}

fn render_latex_resume_pdf(source: &str) -> Vec<u8> {
    let mut body = source;
    if let Some(start) = body.find("\\begin{document}") {
        body = &body[start + 16..];
    }
    if let Some(end) = body.find("\\end{document}") {
        body = &body[..end];
    }

    let mut layout = PdfLayout::a4(50.0, 48.0);
    let accent = (0.12, 0.14, 0.18);
    let ink = (0.08, 0.09, 0.11);
    let muted = (0.38, 0.41, 0.46);

    let mut in_center = false;
    let mut in_itemize = false;
    let mut skip_env = false;
    let mut doc_title = String::from("520pdf LaTeX Resume");

    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            layout.y -= 4.0;
            continue;
        }
        if trimmed.starts_with('%')
            || trimmed.starts_with("\\documentclass")
            || trimmed.starts_with("\\usepackage")
            || trimmed.starts_with("\\geometry")
        {
            continue;
        }
        if trimmed.starts_with("\\vspace") {
            layout.y -= 6.0;
            continue;
        }
        if trimmed.starts_with("\\newpage") {
            layout.new_page();
            continue;
        }
        if trimmed == "\\begin{center}" {
            in_center = true;
            continue;
        }
        if trimmed == "\\end{center}" {
            in_center = false;
            layout.y -= 4.0;
            continue;
        }
        if trimmed == "\\begin{itemize}" || trimmed == "\\begin{enumerate}" {
            in_itemize = true;
            continue;
        }
        if trimmed == "\\end{itemize}" || trimmed == "\\end{enumerate}" {
            in_itemize = false;
            layout.y -= 3.0;
            continue;
        }
        if trimmed.starts_with("\\begin{") {
            skip_env = true;
            continue;
        }
        if trimmed.starts_with("\\end{") {
            skip_env = false;
            continue;
        }
        if skip_env {
            continue;
        }
        if trimmed == "\\hrule" || trimmed == "\\hline" {
            layout.draw_rule((0.68, 0.70, 0.74), 0.7);
            continue;
        }
        if let Some(title) = extract_section_arg(trimmed, "\\section*{")
            .or_else(|| extract_section_arg(trimmed, "\\section{"))
        {
            layout.draw_section_heading(&title, accent);
            continue;
        }
        if let Some(title) = extract_section_arg(trimmed, "\\subsection*{")
            .or_else(|| extract_section_arg(trimmed, "\\subsection{"))
        {
            layout.y -= 2.0;
            layout.draw_wrapped(&title, FontKind::Bold, 10.5, 0.0, ink, 1.35);
            continue;
        }
        if in_itemize && let Some(item_raw) = trimmed.strip_prefix("\\item") {
            let item_txt = clean_latex_inline(item_raw.trim());
            if !item_txt.is_empty() {
                layout.draw_wrapped(
                    &format!("\u{2022} {item_txt}"),
                    FontKind::Regular,
                    9.8,
                    12.0,
                    ink,
                    1.38,
                );
            }
            continue;
        }
        if in_center {
            let is_large = trimmed.contains("\\LARGE")
                || trimmed.contains("\\Large")
                || trimmed.contains("\\huge")
                || trimmed.contains("\\Huge");
            let txt = clean_latex_inline(trimmed);
            if txt.is_empty() {
                continue;
            }
            if is_large {
                if doc_title == "520pdf LaTeX Resume" {
                    doc_title = txt.clone();
                }
                layout.draw_centered(&txt, FontKind::Bold, 18.0, ink);
            } else {
                layout.draw_centered(&txt, FontKind::Regular, 9.5, muted);
            }
            continue;
        }
        if let Some((left_raw, right_raw)) = trimmed.split_once("\\hfill") {
            let left = clean_latex_inline(left_raw);
            let right = clean_latex_inline(right_raw);
            layout.draw_left_right(&left, &right, FontKind::Bold, 10.2, ink, muted);
            continue;
        }

        let is_name_heading =
            trimmed.contains("\\huge") || trimmed.contains("\\Huge") || trimmed.contains("\\LARGE");
        let is_bold = trimmed.starts_with("\\textbf{")
            && find_matching_brace(&trimmed[8..])
                .is_some_and(|e| 8 + e + 1 >= trimmed.len().saturating_sub(4));
        let is_italic = trimmed.starts_with("\\textit{")
            && find_matching_brace(&trimmed[8..])
                .is_some_and(|e| 8 + e + 1 >= trimmed.len().saturating_sub(4));
        let txt = clean_latex_inline(trimmed);
        if txt.is_empty() {
            continue;
        }
        if is_name_heading {
            if doc_title == "520pdf LaTeX Resume" {
                doc_title = txt.clone();
            }
            layout.draw_wrapped(&txt, FontKind::Bold, 18.0, 0.0, ink, 1.3);
        } else if is_bold {
            layout.draw_wrapped(&txt, FontKind::Bold, 10.2, 0.0, ink, 1.38);
        } else if is_italic {
            layout.draw_wrapped(&txt, FontKind::Italic, 9.8, 0.0, muted, 1.35);
        } else {
            layout.draw_wrapped(&txt, FontKind::Regular, 9.8, 0.0, ink, 1.42);
        }
    }

    layout.finish_pdf(&doc_title)
}

fn read_input_bytes(
    files: &[PathBuf],
    inline: Option<&str>,
    kind: &str,
) -> Result<Vec<u8>, String> {
    if let Some(text) = inline
        && !text.trim().is_empty()
    {
        let bytes = text.as_bytes();
        if bytes.len() as u64 > MAX_INPUT {
            return Err(format!("{kind} exceeds the 100 KB limit"));
        }
        return Ok(bytes.to_vec());
    }
    if files.len() != 1 {
        return Err(format!("Upload exactly one {kind} file"));
    }
    let mut bytes = Vec::new();
    File::open(&files[0])
        .map_err(|_| format!("Cannot open {kind}"))?
        .take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("Cannot read {kind}"))?;
    Ok(bytes)
}

/// Generates a resume PDF from JSON (`resume-builder`) or LaTeX (`latex-resume`) in pure Rust.
pub fn run(
    tool: &str,
    files: &[PathBuf],
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let output = dir.join("result.pdf");
    let pdf_bytes = match tool {
        "resume-builder" => {
            let bytes = read_input_bytes(
                files,
                options.get("resume_json").map(String::as_str),
                "resume JSON",
            )?;
            let resume = parse(&bytes)?;
            let photo = options
                .get("photo")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(decode_portrait_photo)
                .transpose()?;
            render_resume_pdf(&resume, options.get("template").map(String::as_str), photo)
        }
        "latex-resume" => {
            let bytes = read_input_bytes(
                files,
                options.get("latex").map(String::as_str),
                "LaTeX resume",
            )?;
            let source =
                std::str::from_utf8(&bytes).map_err(|_| "LaTeX resume must be valid UTF-8")?;
            if source.trim().is_empty() {
                return Err("LaTeX resume content cannot be empty".into());
            }
            render_latex_resume_pdf(source)
        }
        _ => return Err("Unsupported resume tool".into()),
    };
    fs::write(&output, pdf_bytes).map_err(|_| "Cannot write resume PDF")?;
    Ok((output, "application/pdf".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_untrusted_content() {
        let resume =
            parse(br#"{"name":"<script>&\"'","summary":"<img src='file:///secret'>"}"#).unwrap();
        let result = html(&resume);
        assert!(result.contains("&lt;script&gt;&amp;&quot;&#39;"));
        assert!(!result.contains("<img"));
        assert!(result.contains("&lt;img src=&#39;file:///secret&#39;&gt;"));
    }

    #[test]
    fn validates_input() {
        assert!(parse(br#"{"name":"Ada"}"#).is_ok());
        assert!(parse(br#"{"name":" "}"#).is_err());
        assert!(parse(br#"{"name":"Ada","skills":4}"#).is_err());
        assert!(parse(br#"{"name":"Ada","extra":"x"}"#).is_err());
        assert!(parse(&vec![b' '; MAX_INPUT as usize + 1]).is_err());
        assert!(parse(format!("{{\"name\":\"{}\"}}", "a".repeat(201)).as_bytes()).is_err());
    }

    #[test]
    fn pure_rust_resume_builder_renders_valid_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{
            "template": "modern",
            "personalInfo": {
                "fullName": "Ada Lovelace",
                "email": "ada@example.com",
                "phone": "+1 555 0100",
                "address": "London, UK",
                "summary": "Pioneer of analytical computing."
            },
            "experience": [
                {
                    "company": "Analytical Engine Lab",
                    "position": "Lead Mathematician",
                    "location": "London",
                    "startDate": "1842",
                    "endDate": "1852",
                    "description": "Authored the first published algorithm.\nCollaborated with Charles Babbage."
                }
            ],
            "education": [
                {
                    "institution": "University of London",
                    "degree": "Mathematics",
                    "startDate": "1836",
                    "endDate": "1840"
                }
            ],
            "skills": [
                {"name": "Algorithms", "level": "Expert"},
                {"name": "Symbolic Logic", "level": "Advanced"}
            ]
        }"#;
        let opts = HashMap::from([("resume_json".into(), json.into())]);
        let (path, mime) = run("resume-builder", &[], &opts, dir.path()).unwrap();
        assert_eq!(mime, "application/pdf");
        let bytes = fs::read(path).unwrap();
        let doc = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
        assert!(
            bytes.windows(12).any(|w| w == b"Ada Lovelace"),
            "expected resume author name in PDF content"
        );
    }

    #[test]
    fn pure_rust_latex_resume_renders_valid_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let tex = r"\documentclass[11pt,a4paper]{article}
\begin{document}
\begin{center}
{\LARGE \textbf{Grace Hopper}}\\
grace@example.com $\cdot$ Compiler Architect
\end{center}
\hrule
\section*{Experience}
\textbf{Director of Programming} \hfill 1952 -- 1971\\
\begin{itemize}
  \item Built the first linker and COBOL runtime (100\% deterministic)
\end{itemize}
\end{document}";
        let opts = HashMap::from([("latex".into(), tex.into())]);
        let (path, mime) = run("latex-resume", &[], &opts, dir.path()).unwrap();
        assert_eq!(mime, "application/pdf");
        let bytes = fs::read(path).unwrap();
        let doc = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
        assert!(
            bytes.windows(12).any(|w| w == b"Grace Hopper"),
            "expected candidate name in PDF content"
        );
    }

    #[test]
    fn ats_kinobi_resume_with_photo_orgs_other_and_education_first() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbImage::from_pixel(40, 50, image::Rgb([30, 80, 140]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let data_url = format!("data:image/png;base64,{}", BASE64.encode(&png_bytes));

        let json = r#"{
            "documentTitle": "CV Daniel ATS",
            "template": "ats",
            "personalInfo": {
                "fullName": "Daniel Halim",
                "email": "daniel@example.com",
                "phone": "+62 851 5664 4103",
                "linkedin": "linkedin.com/in/daniel",
                "website": "daniel.dev",
                "address": "Surabaya, Indonesia",
                "summary": "Full-stack developer with 3+ years of experience building scalable web applications and SaaS tools."
            },
            "education": [
                {
                    "institution": "Electronic Engineering Polytechnic Institute of Surabaya",
                    "location": "Surabaya, Indonesia",
                    "degree": "Bachelor",
                    "fieldOfStudy": "Informatics Engineering",
                    "startDate": "Jun 2024",
                    "endDate": "Oct 2025",
                    "gpa": "3.71",
                    "maxGpa": "4.00",
                    "description": "Led development of AI-powered SaaS Point of Sale platform."
                }
            ],
            "experience": [
                {
                    "company": "PT Multi Medika Mahadata",
                    "position": "Full Stack Developer",
                    "location": "Surabaya, Indonesia",
                    "companyDescription": "Digital healthcare SaaS platform serving clinics.",
                    "startDate": "Sep 2025",
                    "isCurrent": true,
                    "description": "Designed scalable APIs serving 50+ clinics.\nImproved p95 query latency by 42%."
                }
            ],
            "organizations": [
                {
                    "organization": "HIMIT Alumni Summit",
                    "position": "Chief Executive",
                    "location": "Surabaya",
                    "organizationDescription": "Annual computing alumni forum.",
                    "startDate": "Sep 2022",
                    "endDate": "Sep 2022",
                    "description": "Organized networking session with 100+ participants."
                }
            ],
            "other": [
                {
                    "category": "Hard Skills",
                    "description": "Rust, TypeScript, React, PostgreSQL, Docker"
                },
                {
                    "category": "Project DokterEMR",
                    "year": "2024",
                    "description": "Built clinic medical record workflow end to end."
                }
            ],
            "sectionTitles": {
                "education": "Education",
                "experience": "Work Experience",
                "organizations": "Organisational Experience",
                "other": "Skills & Projects"
            },
            "style": {
                "font": "times",
                "fontSize": 10.0,
                "margin": "small",
                "lineSpacing": 1.25,
                "headerStyle": "ats",
                "alignment": "justify",
                "educationFirst": true,
                "language": "en"
            }
        }"#;
        let opts = HashMap::from([
            ("resume_json".into(), json.into()),
            ("photo".into(), data_url),
        ]);
        let (path, mime) = run("resume-builder", &[], &opts, dir.path()).unwrap();
        assert_eq!(mime, "application/pdf");
        let bytes = fs::read(path).unwrap();
        let doc = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(!doc.get_pages().is_empty());
        assert!(
            bytes.windows(11).any(|w| w == b"Times-Roman"),
            "expected Times-Roman font in ATS PDF"
        );
        assert!(
            bytes.windows(9).any(|w| w == b"DCTDecode"),
            "expected embedded DCTDecode portrait photo in ATS PDF"
        );
        assert!(
            bytes.windows(12).any(|w| w == b"DANIEL HALIM"),
            "expected uppercase candidate name in ATS PDF"
        );
    }
}
