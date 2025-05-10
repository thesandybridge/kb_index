use ignore::WalkBuilder;
use bat::assets::HighlightingAssets;
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;
use syntect::parsing::SyntaxReference;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use std::path::{Path, PathBuf};
use std::collections::HashSet;

pub fn highlight_syntax(code: &str, file_path: &str) -> String {
    let config = config::load_config().expect("failed to load config");
    let theme_name = config.syntax_theme.as_deref().unwrap_or("gruvbox-dark");
    let assets = HighlightingAssets::from_binary();

    let syntax_set = assets
        .get_syntax_set()
        .expect("failed to load bat syntax set");

    let theme = assets.get_theme(theme_name);

    let extension = Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("txt");

    let syntax: &SyntaxReference = syntax_set
        .find_syntax_by_extension(extension)
        .or_else(|| syntax_set.find_syntax_by_first_line(code))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, theme);

    let mut result = String::new();
    for line in LinesWithEndings::from(code) {
        let ranges: Vec<(Style, &str)> =
            h.highlight_line(line, syntax_set).expect("highlighting failed");
        result.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
    }

    result
}

pub fn collect_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let config = config::load_config()?;
    let allowed_exts: HashSet<String> = config
        .file_extensions
        .unwrap_or_else(config::default_extensions)
        .into_iter()
        .collect();

    let mut files = Vec::new();

    if root.is_file() {
        if let Some(ext) = root.extension().and_then(|s| s.to_str()) {
            if allowed_exts.contains(ext) {
                files.push(root.to_path_buf());
            }
        }
    } else {
        let walker = WalkBuilder::new(root)
            .add_custom_ignore_filename(".kbignore") // Optional
            .hidden(false)
            .build();

        for result in walker {
            let entry = result?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if allowed_exts.contains(ext) {
                        files.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    Ok(files)
}

pub fn chunk_text(text: &str) -> Vec<String> {
    text.lines()
        .collect::<Vec<_>>()
        .chunks(10)
        .map(|chunk| chunk.join("\n"))
        .filter(|chunk| !chunk.trim().is_empty())
        .collect()
}

use regex::Regex;

use crate::config;

pub fn render_markdown_highlighted(md: &str) -> String {
    let code_block_re = Regex::new(r"(?s)```(\w*)\n(.*?)```").unwrap();

    let mut out = String::new();
    let mut last = 0;

    for cap in code_block_re.captures_iter(md) {
        let whole = cap.get(0).unwrap();
        let lang = cap.get(1).map(|m| m.as_str()).unwrap_or("text");
        let code = cap.get(2).unwrap().as_str();

        out.push_str(&md[last..whole.start()]);
        out.push_str(&crate::utils::highlight_syntax(code, &format!("fake.{}", lang)));
        last = whole.end();
    }

    out.push_str(&md[last..]);
    out
}
