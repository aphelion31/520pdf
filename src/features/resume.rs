//! Render a resume locally without allowing document resource access.
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_INPUT: u64 = 100 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    experience: String,
    #[serde(default)]
    education: String,
    #[serde(default)]
    skills: String,
}

fn parse(bytes: &[u8]) -> Result<Resume, String> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err("Resume exceeds the 100 KB limit".into());
    }
    let resume: Resume = serde_json::from_slice(bytes).map_err(|_| "Invalid resume JSON")?;
    if resume.name.trim().is_empty() || resume.name.len() > 200 {
        return Err("Name is required and must be at most 200 bytes".into());
    }
    for (name, value, limit) in [
        ("email", &resume.email, 320),
        ("phone", &resume.phone, 100),
        ("summary", &resume.summary, 10_000),
        ("experience", &resume.experience, 40_000),
        ("education", &resume.education, 20_000),
        ("skills", &resume.skills, 10_000),
    ] {
        if value.len() > limit {
            return Err(format!("{name} exceeds its {limit} byte limit"));
        }
    }
    Ok(resume)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn html(resume: &Resume) -> String {
    let contact = [&resume.email, &resume.phone]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| escape(s.trim()))
        .collect::<Vec<_>>()
        .join(" <span class=\"separator\"> / </span> ");
    let sections = [
        ("Profile", &resume.summary),
        ("Experience", &resume.experience),
        ("Education", &resume.education),
        ("Skills", &resume.skills),
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
header {{ border-bottom: 2px solid #315b5d; padding-bottom: 15pt; margin-bottom: 20pt; }}
h1 {{ font-size: 27pt; line-height: 1.1; letter-spacing: -.4pt; margin: 0 0 8pt; color: #172d30; overflow-wrap: anywhere; }}
.contact {{ color: #526368; font-size: 9pt; overflow-wrap: anywhere; }}
.separator {{ color: #9aa9a9; padding: 0 4pt; }}
section {{ margin: 0 0 18pt; }}
h2 {{ color: #315b5d; font-size: 9pt; text-transform: uppercase; letter-spacing: 1.2pt; margin: 0 0 6pt; break-after: avoid; }}
p {{ white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; }}
</style></head><body><header><h1>{}</h1><div class="contact">{contact}</div></header>{sections}</body></html>"#,
        escape(resume.name.trim())
    )
}

/// The caller supplies a private per-request directory and one uploaded JSON file.
pub fn run(
    tool: &str,
    files: &[PathBuf],
    _options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    if tool != "resume-builder" {
        return Err("Unsupported resume tool".into());
    }
    if files.len() != 1 {
        return Err("Upload exactly one resume JSON file".into());
    }
    let mut bytes = Vec::new();
    File::open(&files[0])
        .map_err(|_| "Cannot open resume")?
        .take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read resume")?;
    let resume = parse(&bytes)?;
    let mut source =
        tempfile::NamedTempFile::new_in(dir).map_err(|_| "Cannot create resume document")?;
    source
        .write_all(html(&resume).as_bytes())
        .map_err(|_| "Cannot write resume document")?;
    let output = dir.join("result.pdf");
    let mut child = Command::new("python3")
        .args([
            "-c",
            r#"import sys
from weasyprint import HTML
def deny(url, *args, **kwargs):
    raise ValueError('External resources are disabled')
with open(sys.argv[1], encoding='utf-8') as source:
    HTML(string=source.read(), url_fetcher=deny).write_pdf(sys.argv[2])
"#,
        ])
        .arg(source.path())
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Cannot start python3")?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok((output, "application/pdf".into())),
            Ok(Some(_)) => {
                return Err(
                    "Resume PDF rendering failed; ensure WeasyPrint is installed for python3"
                        .into(),
                );
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cannot monitor resume renderer".into());
            }
            Ok(None) if start.elapsed() >= Duration::from_secs(30) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Resume PDF rendering timed out".into());
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
        }
    }
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
}
