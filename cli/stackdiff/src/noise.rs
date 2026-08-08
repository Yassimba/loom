//! Noise filtering: unresolved calls to language builtins and plumbing
//! methods (`len()`, `clone()`, `isinstance()`, `println!`) hide by default
//! so graphs show your code talking to your code. Tune per repo in
//! `.stackdiff.toml` (`hide` / `show` glob lists); `--noise` shows all;
//! `--noise-report` prints what gets hidden so an agent can iterate.

use std::collections::HashMap;

use serde::Deserialize;

use crate::extract::FunctionIndex;
use crate::types::CallStep;

/// Builtin/plumbing call basenames hidden by default when unresolved.
const BUILTINS: &[&str] = &[
    // python
    "len",
    "str",
    "int",
    "float",
    "bool",
    "list",
    "dict",
    "set",
    "tuple",
    "print",
    "isinstance",
    "issubclass",
    "getattr",
    "setattr",
    "hasattr",
    "enumerate",
    "zip",
    "sorted",
    "reversed",
    "any",
    "all",
    "min",
    "max",
    "sum",
    "abs",
    "round",
    "range",
    "repr",
    "type",
    "super",
    "iter",
    "next",
    "vars",
    "format",
    "id",
    "hash",
    "frozenset",
    "bytes",
    "callable",
    // rust / general plumbing
    "clone",
    "into",
    "from",
    "unwrap",
    "expect",
    "to_string",
    "collect",
    "into_iter",
    "to_owned",
    "as_ref",
    "as_str",
    "as_deref",
    "default",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "map_err",
    "borrow",
    "borrow_mut",
    "as_bytes",
    "chars",
    "to_vec",
    // shared container/string plumbing (rust + js + python methods)
    "push",
    "pop",
    "insert",
    "get",
    "contains",
    "append",
    "extend",
    "remove",
    "keys",
    "values",
    "items",
    "join",
    "split",
    "strip",
    "startswith",
    "endswith",
    "lower",
    "upper",
    "replace",
    "map",
    "filter",
    "forEach",
    "slice",
    "concat",
    "includes",
    "indexOf",
    "trim",
    "add",
    "update",
    "copy",
    "count",
    "index",
    "sort",
    "reverse",
    "find",
    "some",
    "every",
    "reduce",
    "flat",
    "flatMap",
    "entries",
    "freeze",
    "assign",
    "trim_end",
    "trim_start",
    // js/ts globals
    "parseInt",
    "parseFloat",
    "String",
    "Number",
    "Boolean",
    "Array",
    "stringify",
    "parse",
    "log",
    "warn",
    "error",
    "isArray",
    "fromEntries",
    // go
    "cap",
    "make",
    "new",
    "copy",
    "delete",
    "Sprintf",
    "Sprint",
    "Sprintln",
    "Printf",
    "Println",
    "Print",
];

/// Log/format macros hidden by default (signal macros like panic! stay).
const MACROS: &[&str] = &[
    "println!",
    "print!",
    "eprintln!",
    "eprint!",
    "format!",
    "write!",
    "writeln!",
    "vec!",
    "dbg!",
    "info!",
    "debug!",
    "trace!",
    "warn!",
    "error!",
    "assert!",
    "assert_eq!",
    "debug_assert!",
];

/// Per-repo defaults from `.stackdiff.toml` `[defaults]`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Defaults {
    pub max_depth: Option<usize>,
    pub context: Option<usize>,
    pub view: Option<String>,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub theme: Option<String>,
    #[serde(default)]
    pub tests: bool,
}

/// The whole `.stackdiff.toml`: noise globs plus defaults.
#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    hide: Vec<String>,
    #[serde(default)]
    show: Vec<String>,
    #[serde(default)]
    pub defaults: Defaults,
}

pub fn load_file(repo: &std::path::Path) -> FileConfig {
    std::fs::read_to_string(repo.join(".stackdiff.toml"))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

#[derive(Debug, Default)]
pub struct NoiseFilter {
    /// false = --noise: show everything
    pub enabled: bool,
    hide: Vec<String>,
    show: Vec<String>,
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // '*' wildcards only — enough for `Config.*` / `*_helper` shapes.
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut rest = text;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(at) => {
                if index == 0 && at != 0 {
                    return false;
                }
                rest = &rest[at + part.len()..];
            }
            None => return false,
        }
    }
    parts
        .last()
        .is_none_or(|last| last.is_empty() || rest.is_empty())
}

fn basename(key: &str) -> &str {
    key.rsplit("::")
        .next()
        .and_then(|tail| tail.rsplit('.').next())
        .unwrap_or(key)
}

impl NoiseFilter {
    /// Defaults only, enabled — for tests and library callers.
    pub fn default_enabled() -> Self {
        NoiseFilter {
            enabled: true,
            ..Default::default()
        }
    }

    /// Default filter plus any `.stackdiff.toml` at the repo root.
    pub fn load(repo: &std::path::Path, enabled: bool, extra_hide: &[String]) -> Self {
        let config = load_file(repo);
        let mut hide = config.hide;
        hide.extend(extra_hide.iter().cloned());
        NoiseFilter {
            enabled,
            hide,
            show: config.show,
        }
    }

    /// Should this unresolved call hide?
    pub fn hides(&self, key: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let name = basename(key);
        if self
            .show
            .iter()
            .any(|p| glob_match(p, key) || glob_match(p, name))
        {
            return false;
        }
        if self
            .hide
            .iter()
            .any(|p| glob_match(p, key) || glob_match(p, name))
        {
            return true;
        }
        BUILTINS.contains(&name) || MACROS.contains(&key)
    }
}

fn resolved(index: &FunctionIndex, key: &str) -> bool {
    index.contains_key(key) || index.contains_key(&format!("new {key}"))
}

fn scrub_steps(steps: Vec<CallStep>, index: &FunctionIndex, filter: &NoiseFilter) -> Vec<CallStep> {
    let mut kept: Vec<CallStep> = Vec::new();
    for step in steps {
        match step {
            CallStep::Call { key, meta, count } => {
                if !resolved(index, &key) && filter.hides(&key) {
                    continue;
                }
                // Loop collapse: consecutive identical call sites fold to ×N.
                if let Some(CallStep::Call {
                    key: last_key,
                    meta: last_meta,
                    count: last_count,
                }) = kept.last_mut()
                {
                    if *last_key == key && last_meta.args == meta.args {
                        *last_count += count;
                        continue;
                    }
                }
                kept.push(CallStep::Call { key, meta, count });
            }
            CallStep::Branch {
                key,
                label,
                children,
            } => {
                kept.push(CallStep::Branch {
                    key,
                    label,
                    children: scrub_steps(children, index, filter),
                });
            }
        }
    }
    kept
}

/// Strip hidden calls and collapse repeats across every function body.
pub fn scrub_index(index: &mut FunctionIndex, filter: &NoiseFilter) {
    let lookup = index.clone();
    for info in index.all_mut() {
        info.steps = scrub_steps(std::mem::take(&mut info.steps), &lookup, filter);
    }
}

/// Frequency table of unresolved call keys — the agent-facing tuning loop:
/// run `--noise-report`, add globs to `.stackdiff.toml`, re-run.
pub fn report(index: &FunctionIndex, filter: &NoiseFilter) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    fn walk<'a>(
        steps: &'a [CallStep],
        index: &FunctionIndex,
        counts: &mut HashMap<&'a str, usize>,
    ) {
        for step in steps {
            match step {
                CallStep::Call { key, count, .. } => {
                    if !resolved(index, key) {
                        *counts.entry(key.as_str()).or_default() += count;
                    }
                }
                CallStep::Branch { children, .. } => walk(children, index, counts),
            }
        }
    }
    for info in index.values() {
        walk(&info.steps, index, &mut counts);
    }
    let mut rows: Vec<(&str, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    let mut out = vec![
        "Unresolved call keys (most frequent first). `hidden` rows are".to_string(),
        "filtered by default; tune with hide/show globs in .stackdiff.toml:".to_string(),
        String::new(),
    ];
    for (key, count) in rows.iter().take(40) {
        let marker = if filter.hides(key) {
            "hidden "
        } else {
            "shown  "
        };
        out.push(format!("  {marker} {count:>5}×  {key}"));
    }
    if rows.len() > 40 {
        out.push(format!("  … {} more", rows.len() - 40));
    }
    out.join("\n")
}
