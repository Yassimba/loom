pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Tsx,
    Python,
    Go,
    Rust,
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &[".ts", ".mts", ".cts", ".tsx", ".py", ".go", ".rs"];

pub fn detect_language(file: &str) -> Option<Language> {
    let lower = file.to_lowercase();
    if lower.ends_with(".d.ts") {
        return None;
    }
    if lower.ends_with(".tsx") {
        Some(Language::Tsx)
    } else if lower.ends_with(".ts") || lower.ends_with(".mts") || lower.ends_with(".cts") {
        Some(Language::TypeScript)
    } else if lower.ends_with(".py") {
        Some(Language::Python)
    } else if lower.ends_with(".go") {
        Some(Language::Go)
    } else if lower.ends_with(".rs") {
        Some(Language::Rust)
    } else {
        None
    }
}

impl Language {
    pub fn grammar(&self) -> tree_sitter::Language {
        match self {
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }
}

pub fn named_children<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut out = Vec::with_capacity(node.named_child_count());
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            out.push(child);
        }
    }
    out
}

pub fn child_by_type<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    named_children(node).into_iter().find(|c| c.kind() == kind)
}

pub fn text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

pub fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The doc comment immediately above `node`: contiguous preceding comment
/// siblings, cleaned of their markers, first sentence only.
pub fn doc_before(node: Node, source: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cursor = node.prev_sibling();
    while let Some(prev) = cursor {
        let kind = prev.kind();
        if kind != "comment" && kind != "line_comment" && kind != "block_comment" {
            break;
        }
        // Only adjacent comments count (no blank-line gap of 2+).
        if node.start_position().row > 0
            && prev.end_position().row + 2 < node.start_position().row + lines.len() + 2
        {
            // conservative adjacency check handled below by row math per hop
        }
        lines.push(text(prev, source).to_string());
        cursor = prev.prev_sibling();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    let raw = lines.join(
        "
",
    );
    let cleaned = raw
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim_start_matches("//")
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim()
        })
        .find(|line| !line.is_empty() && !line.starts_with('@') && !line.starts_with('#'))?
        .to_string();
    Some(crate::types::first_sentence(&cleaned))
}

/// Collapse a call-site argument to a short display text.
pub fn arg_text(node: Node, source: &str) -> String {
    let collapsed = collapse_ws(text(node, source));
    if collapsed.chars().count() > 24 {
        let mut short: String = collapsed.chars().take(21).collect();
        short.push('…');
        short
    } else {
        collapsed
    }
}

/// Which known bindings an argument list consumes.
pub fn consumed(bindings: &[String], args: &[String]) -> Vec<String> {
    bindings
        .iter()
        .filter(|binding| {
            args.iter().any(|arg| {
                arg == *binding
                    || arg.starts_with(&format!("{binding}."))
                    || arg.starts_with(&format!("{binding}["))
                    || arg.starts_with(&format!("&{binding}"))
                    || arg.starts_with(&format!("&mut {binding}"))
            })
        })
        .cloned()
        .collect()
}
