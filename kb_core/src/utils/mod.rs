use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use two_face::theme::extra;
use walkdir::WalkDir;

pub fn highlight_syntax(code: &str, file_path: &str) -> String {
    let ps = SyntaxSet::load_defaults_newlines();
    let theme_set = extra();
    let theme = theme_set.get(two_face::theme::EmbeddedThemeName::GruvboxDark);

    let syntax = ps
        .find_syntax_for_file(file_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, theme);
    let mut result = String::new();

    for line in LinesWithEndings::from(code) {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
        result.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
    }

    result
}

pub fn collect_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if root.is_file() {
        files.push(root.to_path_buf());
    } else {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file()
                && matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("md" | "rs" | "tsx" | "ts" | "js" | "jsx")
                )
            {
                files.push(path.to_path_buf());
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
