pub mod go;
pub mod python;
pub mod typescript;

use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Tsx,
    Python,
    Go,
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &[".ts", ".mts", ".cts", ".tsx", ".py", ".go"];

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
