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

/// Callable index: every definition per key, resolved against the caller's
/// file so a Python `resolve` never links to a Rust `resolve`.
#[derive(Debug, Clone, Default)]
pub struct FunctionIndex {
    by_key: HashMap<String, Vec<FunctionInfo>>,
}

fn extension(file: &str) -> &str {
    file.rsplit('.').next().unwrap_or("")
}

fn shared_components(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| x == y).count()
}

impl FunctionIndex {
    pub fn contains_key(&self, key: &str) -> bool {
        self.by_key.contains_key(key)
    }

    /// The candidate closest to the caller: same file, then deepest shared
    /// directory, then same language; without a caller, the first seen.
    pub fn best<'a>(&'a self, key: &str, caller_file: Option<&str>) -> Option<&'a FunctionInfo> {
        let candidates = self.by_key.get(key)?;
        let Some(caller) = caller_file else {
            return candidates.first();
        };
        candidates.iter().max_by_key(|info| {
            let mut score: i64 = 0;
            if info.file == caller {
                score += 1_000_000;
            }
            score += shared_components(&info.file, caller) as i64 * 1_000;
            if extension(&info.file) == extension(caller) {
                score += 100_000;
            }
            score
        })
    }

    pub fn get(&self, key: &str) -> Option<&FunctionInfo> {
        self.best(key, None)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.by_key.keys()
    }

    /// First definition per key — entry listing and inference walk this.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &FunctionInfo)> {
        self.by_key
            .iter()
            .filter_map(|(key, infos)| infos.first().map(|info| (key, info)))
    }

    /// Every definition, including same-key shadows.
    pub fn all(&self) -> impl Iterator<Item = &FunctionInfo> {
        self.by_key.values().flatten()
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut FunctionInfo> {
        self.by_key.values_mut().flatten()
    }

    pub fn values(&self) -> impl Iterator<Item = &FunctionInfo> {
        self.by_key.values().filter_map(|infos| infos.first())
    }
}

pub fn build_index(functions: Vec<FunctionInfo>) -> FunctionIndex {
    let mut index = FunctionIndex::default();
    for func in functions {
        if func.key.ends_with(".constructor") {
            let class_name = func.key.trim_end_matches(".constructor").to_string();
            let new_key = format!("new {class_name}");
            let mut alias = func.clone();
            alias.key = new_key.clone();
            alias.label = format!("new {class_name}()");
            index.by_key.entry(new_key).or_default().push(alias);
        }
        index.by_key.entry(func.key.clone()).or_default().push(func);
    }
    index
}
