use walkdir::WalkDir;
use bat::assets::HighlightingAssets;
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;
use syntect::parsing::SyntaxReference;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

pub fn pipe_to_bat(content: &str, file_path: &str) -> std::io::Result<()> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("txt");

    let mut child = Command::new("bat")
        .arg("--language")
        .arg(extension)
        .arg("--style")
        .arg("plain") // Or "full" or "numbers" if you want line numbers and headers
        .arg("--paging")
        .arg("always")
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}

pub fn highlight_syntax(code: &str, file_path: &str) -> String {
    let assets = HighlightingAssets::from_binary();

    // Unwrap the SyntaxSet and ThemeSet properly
    let syntax_set = assets
        .get_syntax_set()
        .expect("failed to load bat syntax set");

    // If your theme isn't in the list, this will panic. Replace with one from `bat --list-themes` if needed.
    let theme = assets.get_theme("gruvbox-dark");

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
