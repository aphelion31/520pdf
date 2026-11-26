//! Equation images use Matplotlib mathtext, a subset of LaTeX, not full LaTeX.
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn run(
    tool: &str,
    files: &[PathBuf],
    options: &HashMap<String, String>,
    dir: &Path,
) -> Result<(PathBuf, String), String> {
    if tool != "equation-to-image" {
        return Err("Unknown equation tool".into());
    }
    let expr_owned;
    let expression = if let Some(expr) = options.get("expression").filter(|s| !s.trim().is_empty())
    {
        if expr.len() > 16_000 {
            return Err("Equation exceeds 4000 characters".into());
        }
        expr.as_str()
    } else {
        if files.len() != 1 {
            return Err("Upload exactly one .txt file".into());
        }
        if !files[0]
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("txt"))
        {
            return Err("Upload a .txt file".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(&files[0])
            .map_err(|_| "Cannot read equation file")?
            .take(16_001)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read equation file")?;
        if bytes.len() > 16_000 {
            return Err("Equation exceeds 4000 characters".into());
        }
        expr_owned = String::from_utf8(bytes).map_err(|_| "Equation must be UTF-8")?;
        &expr_owned
    };
    let size = font_size(options)?;
    validate_expression(expression)?;
    fs::create_dir_all(dir).map_err(|_| "Cannot prepare output directory")?;
    let dir = dir
        .canonicalize()
        .map_err(|_| "Cannot access output directory")?;
    let out = dir.join("equation.png");
    // Avoid accepting stale output after a failed or ineffective renderer.
    if out.exists() {
        return Err("Equation output already exists".into());
    }
    super::conversions::command("python3", &[
        "-c".into(),
        r#"import sys
import matplotlib
matplotlib.use('Agg')
matplotlib.rcParams['text.usetex'] = False
from matplotlib.font_manager import FontProperties
from matplotlib.mathtext import math_to_image
math_to_image('$' + sys.argv[1] + '$', sys.argv[2], prop=FontProperties(size=int(sys.argv[3])), format='png', dpi=180)
"#.into(),
        expression.into(), out.to_string_lossy().into_owned(), size.to_string(),
    ], &dir).map_err(|_| "Could not render equation. Use valid Matplotlib mathtext (a subset of LaTeX), without $ delimiters.".to_string())?;
    if !out.is_file() || fs::metadata(&out).map_or(true, |m| m.len() == 0) {
        return Err("Equation renderer produced no image".into());
    }
    Ok((out, "image/png".into()))
}

fn validate_expression(expression: &str) -> Result<(), String> {
    if expression.trim().is_empty() {
        return Err("Equation must not be empty".into());
    }
    if expression.chars().count() > 4000 {
        return Err("Equation exceeds 4000 characters".into());
    }
    if expression.contains('$') {
        return Err("Do not include $ delimiters in the equation".into());
    }
    if expression.contains('\0') {
        return Err("Equation contains an invalid character".into());
    }
    Ok(())
}

fn font_size(options: &HashMap<String, String>) -> Result<u8, String> {
    let size = options
        .get("font_size")
        .map(String::as_str)
        .unwrap_or("24")
        .parse::<u8>()
        .map_err(|_| "font_size must be an integer between 12 and 96")?;
    if !(12..=96).contains(&size) {
        return Err("font_size must be an integer between 12 and 96".into());
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_expression_and_font_size() {
        assert!(validate_expression("x^2").is_ok());
        assert!(validate_expression(&"é".repeat(4000)).is_ok());
        assert!(validate_expression(&"x".repeat(4001)).is_err());
        assert!(validate_expression("  ").is_err());
        assert!(validate_expression("$x$").is_err());
        assert!(validate_expression("x\0y").is_err());
        assert_eq!(font_size(&HashMap::new()).unwrap(), 24);
        for size in ["12", "96"] {
            assert!(font_size(&HashMap::from([("font_size".into(), size.into())])).is_ok());
        }
        for size in ["11", "97", "abc"] {
            assert!(font_size(&HashMap::from([("font_size".into(), size.into())])).is_err());
        }
    }
}
