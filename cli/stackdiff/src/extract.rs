//! Parse a source file with the right grammar and run its language extractor.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tree_sitter::Parser;

use crate::lang::{self, detect_language, Language};
use crate::types::{FunctionInfo, TypeInfo};

pub fn extract_functions(file: &str, source: &str) -> Result<Vec<FunctionInfo>> {
    let Some(language) = detect_language(file) else {
        return Ok(Vec::new());
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .context("failed to load grammar")?;
    let tree = parser
        .parse(source, None)
        .with_context(|| format!("failed to parse {file}"))?;

    Ok(match language {
        Language::TypeScript | Language::Tsx => lang::typescript::extract(file, source, &tree),
        Language::Python => lang::python::extract(file, source, &tree),
        Language::Go => lang::go::extract(file, source, &tree),
        Language::Rust => lang::rust::extract(file, source, &tree),
    })
}

/// Type definitions in a file, for the --er view.
pub fn extract_types(file: &str, source: &str) -> Result<Vec<TypeInfo>> {
    let Some(language) = detect_language(file) else {
        return Ok(Vec::new());
    };
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .context("failed to load grammar")?;
    let tree = parser
        .parse(source, None)
        .with_context(|| format!("failed to parse {file}"))?;
    Ok(match language {
        Language::TypeScript | Language::Tsx => lang::typescript::extract_types(source, &tree),
        Language::Python => lang::python::extract_types(source, &tree),
        Language::Go => lang::go::extract_types(source, &tree),
        Language::Rust => lang::rust::extract_types(source, &tree),
    })
}

pub type FunctionIndex = HashMap<String, FunctionInfo>;

pub fn build_index(functions: Vec<FunctionInfo>) -> FunctionIndex {
    let mut index: FunctionIndex = HashMap::new();
    for func in functions {
        if func.key.ends_with(".constructor") {
            let class_name = func.key.trim_end_matches(".constructor").to_string();
            let new_key = format!("new {class_name}");
            index.entry(new_key.clone()).or_insert_with(|| {
                let mut alias = func.clone();
                alias.key = new_key;
                alias.label = format!("new {class_name}()");
                alias
            });
        }
        index.entry(func.key.clone()).or_insert(func);
    }
    index
}
