//! Pure-Rust resume and LaTeX resume PDF generator using `pdf-writer`.
//! Generates A4 PDFs directly without executing external TeX or browser engines.
use pdf_writer::{Content, Name, Pdf, Rect, Ref, Str, TextStr};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const MAX_INPUT: u64 = 100 * 1024;
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
    #[serde(default, alias = "startDate")]
    start_date: String,
    #[serde(default, alias = "endDate")]
    end_date: String,
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
    degree: String,
    #[serde(default, alias = "fieldOfStudy")]
    field_of_study: String,
    #[serde(default, alias = "startDate")]
    start_date: String,
    #[serde(default, alias = "endDate")]
    end_date: String,
    #[serde(default)]
    description: String,
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
    skills: SkillsInput,
    #[serde(default)]
    template: String,
    #[serde(default, alias = "personalInfo")]
    personal_info: Option<PersonalInfo>,
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

    fn website(&self) -> &str {
        if !self.website.trim().is_empty() {
            self.website.trim()
        } else if !self.linkedin.trim().is_empty() {
            self.linkedin.trim()
        } else if let Some(info) = &self.personal_info {
            if !info.website.trim().is_empty() {
                info.website.trim()
            } else {
                info.linkedin.trim()
            }
        } else {
            ""
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

#[derive(Clone, Copy)]
pub(crate) enum FontKind {
    Regular,
    Bold,
    Italic,
}

impl FontKind {
    fn name(self) -> Name<'static> {
        match self {
            Self::Regular => Name(b"F1"),
            Self::Bold => Name(b"F2"),
            Self::Italic => Name(b"F3"),
        }
    }

    fn avg_width_factor(self) -> f32 {
        match self {
            Self::Regular => 0.49,
            Self::Bold => 0.52,
            Self::Italic => 0.49,
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
            c if (' '..='~').contains(&c) => c as u8,
            c if (0xA0..=0xFF).contains(&(c as u32)) => c as u8,
            '\t' => b' ',
            _ => b'?',
        })
        .collect()
}

pub(crate) fn approx_width(text: &str, size: f32, font: FontKind) -> f32 {
    let mut units = 0.0f32;
    for ch in text.chars() {
        units += match ch {
            ' ' | 'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '|' => 0.28,
            'f' | 'j' | 'r' | 't' | '-' | '/' | '(' | ')' => 0.34,
            'm' | 'w' | 'M' | 'W' => 0.78,
            c if c.is_ascii_uppercase() => 0.64,
            _ => font.avg_width_factor(),
        };
    }
    units * size
}

pub(crate) fn wrap_text(text: &str, max_width: f32, size: f32, font: FontKind) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let trimmed = raw_line.trim_end();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in trimmed.split_whitespace() {
            if current.is_empty() {
                if approx_width(word, size, font) <= max_width {
                    current.push_str(word);
                } else {
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        chunk.push(ch);
                        if approx_width(&chunk, size, font) >= max_width {
                            lines.push(std::mem::take(&mut chunk));
                        }
                    }
                    current = chunk;
                }
            } else {
                let candidate = format!("{current} {word}");
                if approx_width(&candidate, size, font) <= max_width {
                    current = candidate;
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(word);
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(crate) struct PdfLayout {
    pages: Vec<Content>,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) margin_x: f32,
    pub(crate) margin_y: f32,
    pub(crate) y: f32,
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
        }
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

    pub(crate) fn draw_text_at(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font: FontKind,
        size: f32,
        color: (f32, f32, f32),
    ) {
        if text.is_empty() {
            return;
        }
        let c = self.content();
        c.set_fill_rgb(color.0, color.1, color.2);
        c.begin_text();
        c.set_font(font.name(), size);
        c.next_line(x, y);
        let bytes = to_win_ansi(text);
        c.show(Str(&bytes));
        c.end_text();
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
        let lines = wrap_text(text, self.avail(indent), size, font);
        let leading = size * leading_mult;
        for line in lines {
            self.ensure_space(leading);
            let x = self.margin_x + indent;
            let y = self.y - size;
            self.draw_text_at(&line, x, y, font, size, color);
            self.y -= leading;
        }
    }

    pub(crate) fn draw_centered(
        &mut self,
        text: &str,
        font: FontKind,
        size: f32,
        color: (f32, f32, f32),
    ) {
        let lines = wrap_text(text, self.avail(0.0), size, font);
        let leading = size * 1.35;
        for line in lines {
            self.ensure_space(leading);
            let w = approx_width(&line, size, font);
            let x = ((self.width - w) * 0.5).max(self.margin_x);
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
            let rw = approx_width(right, size * 0.92, FontKind::Regular);
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

    pub(crate) fn finish_pdf(self, title: &str) -> Vec<u8> {
        let mut pdf = Pdf::new();
        let catalog_id = Ref::new(1);
        let pages_id = Ref::new(2);
        let info_id = Ref::new(3);
        let f1_id = Ref::new(4);
        let f2_id = Ref::new(5);
        let f3_id = Ref::new(6);

        pdf.catalog(catalog_id).pages(pages_id);
        {
            let mut info = pdf.document_info(info_id);
            info.title(TextStr(title));
            info.producer(TextStr("520pdf"));
            info.creator(TextStr("520pdf"));
        }
        for (id, base) in [
            (f1_id, b"Helvetica".as_slice()),
            (f2_id, b"Helvetica-Bold".as_slice()),
            (f3_id, b"Helvetica-Oblique".as_slice()),
        ] {
            let mut font = pdf.type1_font(id);
            font.base_font(Name(base));
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }

        let mut page_ids = Vec::with_capacity(self.pages.len());
        let mut next_id = 7i32;
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
        for (page_id, content_id, content) in page_pairs {
            let data = content.finish();
            pdf.stream(content_id, &data);
            let mut page = pdf.page(page_id);
            page.parent(pages_id);
            page.media_box(Rect::new(0.0, 0.0, self.width, self.height));
            page.contents(content_id);
            let mut res = page.resources();
            let mut fonts = res.fonts();
            fonts.pair(Name(b"F1"), f1_id);
            fonts.pair(Name(b"F2"), f2_id);
            fonts.pair(Name(b"F3"), f3_id);
        }

        pdf.finish()
    }
}

fn template_palette(template: &str) -> ((f32, f32, f32), (f32, f32, f32), bool) {
    match template.trim().to_ascii_lowercase().as_str() {
        "modern" => ((0.78, 0.16, 0.21), (0.14, 0.16, 0.20), false),
        "professional" | "classic" => ((0.06, 0.09, 0.14), (0.12, 0.14, 0.18), true),
        _ => ((0.18, 0.20, 0.24), (0.16, 0.18, 0.21), false),
    }
}

fn render_resume_pdf(resume: &Resume, template_override: Option<&str>) -> Vec<u8> {
    let tpl = template_override
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(resume.template.as_str());
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
            render_resume_pdf(&resume, options.get("template").map(String::as_str))
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
}
