//! Test helpers ported from calldiff's vitest fixtures:
//! a +/- file diff in, ASCII callstack diff out.

use stackdiff::calltree::build_call_tree;
use stackdiff::diff::diff_trees;
use stackdiff::extract::{build_index, extract_functions};
use stackdiff::render::{render_diff, RenderOptions};

/// Outdent +/- diff strings (file diffs and callstack diffs).
///
/// Unlike plain outdent, lines that start with `+` / `-` set the indent
/// level — so markers aren't eaten when unchanged lines are indented further.
pub fn diff_outdent(text: &str) -> String {
    let text = text.strip_prefix('\n').unwrap_or(text);
    let text = text.strip_suffix('\n').unwrap_or(text);

    let lines: Vec<&str> = text.split('\n').collect();

    let leading_ws = |line: &str| line.len() - line.trim_start_matches(' ').len();
    let marker_indent = |line: &str| {
        let ws = leading_ws(line);
        let rest = &line[ws..];
        (rest.starts_with('+') || rest.starts_with('-')).then_some(ws)
    };

    let marker_indents: Vec<usize> = lines.iter().filter_map(|l| marker_indent(l)).collect();
    let all_indents: Vec<usize> = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| leading_ws(l))
        .collect();

    let indent = if !marker_indents.is_empty() {
        *marker_indents.iter().min().unwrap()
    } else if !all_indents.is_empty() {
        *all_indents.iter().min().unwrap()
    } else {
        0
    };

    let out: Vec<String> = lines
        .iter()
        .map(|line| {
            let cut = indent.min(leading_ws(line));
            line[cut..].to_string()
        })
        .collect();
    out.join("\n").trim_end_matches('\n').to_string()
}

/// Reconstruct before/after file contents from a unified-style diff.
pub fn sources_from_file_diff(file_diff: &str) -> (String, String) {
    let mut before: Vec<String> = Vec::new();
    let mut after: Vec<String> = Vec::new();

    for raw in file_diff.split('\n') {
        if raw.starts_with("---")
            || raw.starts_with("+++")
            || raw.starts_with("@@")
            || raw.starts_with("diff ")
            || raw.starts_with("index ")
            || raw.starts_with('\\')
        {
            continue;
        }

        let marker = raw.chars().next();
        let content: String = raw.chars().skip(1).collect();

        match marker {
            Some('-') => before.push(content),
            Some('+') => after.push(content),
            Some(' ') => {
                before.push(content.clone());
                after.push(content);
            }
            _ => {
                before.push(raw.to_string());
                after.push(raw.to_string());
            }
        }
    }

    (before.join("\n"), after.join("\n"))
}

fn snapshot_names(file: &str) -> (String, String) {
    let (stem, ext) = match file.rfind('.') {
        Some(dot) => (&file[..dot], &file[dot..]),
        None => (file, ".ts"),
    };
    let stem = if stem.is_empty() { "file" } else { stem };
    (format!("{stem}.before{ext}"), format!("{stem}.after{ext}"))
}

pub fn callstack_diff_with(file_diff: &str, entry: &str, file: &str, max_depth: usize) -> String {
    let outdented = diff_outdent(file_diff);
    let (before_name, after_name) = snapshot_names(file);
    let (before_source, after_source) = sources_from_file_diff(&outdented);

    let before = build_index(extract_functions(&before_name, &before_source).unwrap());
    let after = build_index(extract_functions(&after_name, &after_source).unwrap());

    let before_tree = build_call_tree(entry, &before, max_depth);
    let after_tree = build_call_tree(entry, &after, max_depth);
    let diff = diff_trees(&before_tree, &after_tree);
    render_diff(&diff, RenderOptions { color: false })
}

pub fn callstack_diff(file_diff: &str, entry: &str) -> String {
    callstack_diff_with(file_diff, entry, "file.ts", 12)
}

pub fn expect_callstack(file_diff: &str, entry: &str, expected: &str) {
    pretty_eq(&callstack_diff(file_diff, entry), &diff_outdent(expected));
}

pub fn expect_callstack_in(file_diff: &str, entry: &str, file: &str, expected: &str) {
    pretty_eq(
        &callstack_diff_with(file_diff, entry, file, 12),
        &diff_outdent(expected),
    );
}

pub fn expect_callstack_depth(file_diff: &str, entry: &str, max_depth: usize, expected: &str) {
    pretty_eq(
        &callstack_diff_with(file_diff, entry, "file.ts", max_depth),
        &diff_outdent(expected),
    );
}

pub fn pretty_eq(actual: &str, expected: &str) {
    assert!(
        actual == expected,
        "callstack mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}\n"
    );
}
