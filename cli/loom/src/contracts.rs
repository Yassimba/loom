//! Read-only structural validation and discovery of code contracts: directory
//! contracts in `CONTRACTS` files and declaration contracts in doc comments.

use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// What a contract governs: a whole directory, or one declaration's lines.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Directory,
    Declaration { name: String, lines: (usize, usize) },
}

#[derive(Clone, Debug, Serialize)]
pub struct Contract {
    pub id: String,
    pub metadata: Vec<(String, Vec<String>)>,
    pub prose: String,
    pub path: PathBuf,
    pub line: usize,
    pub scope: Scope,
}

/// Contracts answering one `list` request; `by_id` distinguishes an
/// identifier lookup (a miss is a finding) from a path listing (a miss is not).
#[derive(Debug, Serialize)]
pub struct ListReport {
    pub contracts: Vec<Contract>,
    #[serde(skip)]
    pub by_id: bool,
}

/// How one language marks the doc comments a contract can live in.
enum Style {
    Line(&'static [&'static str]),
    Block(&'static str, &'static str),
    /// A docstring sits inside the declaration it documents, not above it.
    Docstring(&'static str),
}

fn comment_style(extension: &str) -> Option<Style> {
    match extension {
        "rs" => Some(Style::Line(&["///", "//!", "//"])),
        "py" | "pyi" => Some(Style::Docstring("\"\"\"")),
        "ts" | "tsx" | "mts" | "cts" => Some(Style::Block("/*", "*/")),
        _ => None,
    }
}

fn supported_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(comment_style)
        .is_some()
}

/// A file contracts can live in: a `CONTRACTS` file or a supported source file.
fn contract_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "CONTRACTS") || supported_source(path)
}

/// The repository root for the current working directory.
fn cwd_root() -> Result<PathBuf> {
    Ok(crate::project_root(
        &std::env::current_dir().context("current directory is unavailable")?,
    ))
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub checked: usize,
    pub diagnostics: Vec<Diagnostic>,
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, ',' | ':' | '[' | ']'))
}

fn directive(line: &str, path: &Path, number: usize) -> Result<Contract, &'static str> {
    let mut rest = line
        .strip_prefix("@cc ")
        .ok_or("expected @cc followed by a space and contract ID")?
        .trim_start_matches(' ');
    let mut metadata = Vec::new();
    if let Some(attributes) = rest.strip_prefix('[') {
        let (attributes, tail) = attributes.split_once(']').ok_or("unclosed metadata")?;
        for attribute in attributes.split(',') {
            let (key, value) = attribute
                .split_once(':')
                .ok_or("expected metadata key:value")?;
            if !token(key) || !token(value) {
                return Err("metadata keys and values must be non-empty tokens");
            }
            metadata.push((key.into(), value.split(';').map(String::from).collect()));
        }
        rest = tail
            .strip_prefix(' ')
            .ok_or("expected a space after metadata")?
            .trim_start_matches(' ');
    }
    if !token(rest) {
        return Err("expected one non-empty contract ID token");
    }
    Ok(Contract {
        id: rest.into(),
        metadata,
        prose: String::new(),
        path: path.into(),
        line: number,
        scope: Scope::Directory,
    })
}

/// Comment content per line: `Some(text)` inside a doc comment, `None` for code.
fn comment_lines<'a>(text: &'a str, style: &Style) -> Vec<Option<&'a str>> {
    let mut open = false;
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            match style {
                Style::Line(prefixes) => prefixes
                    .iter()
                    .find_map(|prefix| trimmed.strip_prefix(prefix))
                    .map(str::trim),
                Style::Docstring(quote) => {
                    let fences = trimmed.matches(quote).count();
                    let inside = open;
                    if fences % 2 == 1 {
                        open = !open;
                    }
                    (inside || fences > 0).then(|| trimmed.trim_matches('"').trim())
                }
                Style::Block(start, end) => {
                    let opened = open;
                    let mut content = trimmed;
                    if !open {
                        match content.find(start) {
                            Some(index) => {
                                open = true;
                                content = &content[index + start.len()..];
                            }
                            None => return None,
                        }
                    }
                    if let Some(index) = content.find(end) {
                        // A one-line docstring both opens and closes here.
                        if opened || !content.is_empty() {
                            open = false;
                            content = &content[..index];
                        }
                    }
                    Some(content.trim_start_matches('*').trim())
                }
            }
        })
        .collect()
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Is this line the declaration a contract can attach to?
fn is_declaration(lines: &[&str], comments: &[Option<&str>], index: usize) -> bool {
    let trimmed = lines[index].trim();
    comments[index].is_none()
        && !trimmed.is_empty()
        && !trimmed.starts_with("#[")
        && !trimmed.starts_with('@')
}

/// The declaration starting at `start`: its name and the lines it governs,
/// which run until the next code line at equal or lower indentation.
///
/// ponytail: indentation heuristic, not a parse tree. Upgrade to tree-sitter
/// behind this same signature if a real grammar becomes necessary.
fn governed(lines: &[&str], comments: &[Option<&str>], start: usize) -> (String, usize, usize) {
    let depth = indentation(lines[start]);
    let mut end = start;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if indentation(line) <= depth && comments[index].is_none() {
            break;
        }
        end = index;
    }
    let mut name = lines[start].trim().to_owned();
    name.truncate(name.char_indices().nth(80).map_or(name.len(), |(at, _)| at));
    (name, start + 1, end + 1)
}

/**
@cc [owner:Yassimba] contracts-attachment-declaration-identity
An attached contract records the declaration it governs, and duplicate-ID detection for attached contracts applies within one declaration rather than across a file.
*/
fn attach(text: &str, path: &Path) -> (Vec<Contract>, Vec<Diagnostic>) {
    let Some(style) = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(comment_style)
    else {
        return (Vec::new(), Vec::new());
    };
    let lines: Vec<_> = text.lines().collect();
    let comments = comment_lines(text, &style);
    let mut contracts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(content) = comments[index] else {
            index += 1;
            continue;
        };
        if !is_directive(content) {
            index += 1;
            continue;
        }
        let block = (index + 1..lines.len())
            .take_while(|&next| comments[next].is_some())
            .take_while(|&next| !is_directive(comments[next].unwrap_or_default()))
            .last()
            .unwrap_or(index);
        match directive(content, path, index + 1) {
            Ok(mut contract) => {
                contract.prose = (index + 1..=block)
                    .filter_map(|line| comments[line])
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_owned();
                if contract.prose.is_empty() {
                    // The next directive is in this same comment when no code
                    // line separates them, which the format does not allow.
                    let shared = comments
                        .get(block + 1)
                        .is_some_and(|next| next.is_some_and(is_directive));
                    diagnostics.push(Diagnostic {
                        path: path.into(),
                        line: index + 1,
                        message: if shared {
                            "a documentation comment carries one @cc directive".into()
                        } else {
                            "contract prose must be non-empty".to_string()
                        },
                    });
                }
                // A docstring sits inside its declaration; other comments precede it.
                let attached = match style {
                    Style::Docstring(_) => (0..index)
                        .rev()
                        .find(|&at| is_declaration(&lines, &comments, at)),
                    _ => (block + 1..lines.len()).find(|&at| is_declaration(&lines, &comments, at)),
                }
                .map(|start| governed(&lines, &comments, start));
                match attached {
                    Some((name, first, last)) => {
                        contract.scope = Scope::Declaration {
                            name,
                            lines: (first, last),
                        };
                        contracts.push(contract);
                    }
                    None => diagnostics.push(Diagnostic {
                        path: path.into(),
                        line: index + 1,
                        message: "contract has no following declaration".into(),
                    }),
                }
            }
            Err(message) => diagnostics.push(Diagnostic {
                path: path.into(),
                line: index + 1,
                message: message.into(),
            }),
        }
        index = block + 1;
    }
    diagnostics.extend(declaration_duplicates(&contracts));
    (contracts, diagnostics)
}

fn is_directive(content: &str) -> bool {
    content
        .strip_prefix("@cc")
        .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
}

/// Attached identity is the declaration plus the ID: the same ID on two
/// declarations is valid, twice on one declaration is not.
fn declaration_duplicates(contracts: &[Contract]) -> Vec<Diagnostic> {
    let mut first = BTreeMap::<(&Scope, &str), usize>::new();
    let mut diagnostics = Vec::new();
    for contract in contracts {
        match first.entry((&contract.scope, &contract.id)) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(contract.line);
            }
            std::collections::btree_map::Entry::Occupied(entry) => diagnostics.push(Diagnostic {
                path: contract.path.clone(),
                line: contract.line,
                message: format!(
                    "duplicate contract id `{}`; first declared at {}:{}",
                    contract.id,
                    contract.path.display(),
                    entry.get()
                ),
            }),
        }
    }
    diagnostics
}

fn parse(text: &str, path: &Path) -> (Vec<Contract>, Vec<Diagnostic>) {
    let lines: Vec<_> = text.lines().collect();
    let mut contracts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let line = lines[start].trim();
        if line.is_empty() {
            start += 1;
            continue;
        }
        let end = (start + 1..lines.len())
            .find(|&index| is_directive(lines[index].trim_start()))
            .unwrap_or(lines.len());
        match directive(line, path, start + 1) {
            Ok(mut contract) => {
                contract.prose = lines[start + 1..end].join("\n").trim().into();
                if contract.prose.is_empty() {
                    diagnostics.push(Diagnostic {
                        path: path.into(),
                        line: start + 1,
                        message: "contract prose must be non-empty".into(),
                    });
                }
                contracts.push(contract);
            }
            Err(message) => diagnostics.push(Diagnostic {
                path: path.into(),
                line: start + 1,
                message: message.into(),
            }),
        }
        start = end;
    }
    (contracts, diagnostics)
}

fn collect(directory: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    if directory.file_name().is_some_and(|name| name == ".git") {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .with_context(|| format!("cannot read directory {}", directory.display()))?
    {
        let entry =
            entry.with_context(|| format!("cannot read entry in {}", directory.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .with_context(|| format!("cannot inspect {}", path.display()))?;
        // Never follow links: a contract tree must not read outside its selected scope.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(&path, files)?;
        } else if contract_file(&path) {
            ensure!(kind.is_file(), "not a regular file: {}", path.display());
            files.insert(path);
        }
    }
    Ok(())
}

/// Only directory contracts share an identifier namespace across files.
fn duplicate_diagnostics(contracts: &[Contract]) -> Vec<Diagnostic> {
    let contracts = contracts
        .iter()
        .filter(|contract| contract.scope == Scope::Directory);
    let mut first = BTreeMap::<PathBuf, BTreeMap<String, usize>>::new();
    for contract in contracts.clone() {
        first
            .entry(contract.path.clone())
            .or_default()
            .entry(contract.id.clone())
            .or_insert(contract.line);
    }
    let mut diagnostics = Vec::new();
    for contract in contracts {
        // Look only in this file and its ancestor chain, never sibling trees.
        let previous = contract
            .path
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .find_map(|directory| {
                let path = directory.join("CONTRACTS");
                let line = *first.get(&path)?.get(&contract.id)?;
                (path != contract.path || line < contract.line).then_some((path, line))
            });
        if let Some((path, line)) = previous {
            diagnostics.push(Diagnostic {
                path: contract.path.clone(),
                line: contract.line,
                message: format!(
                    "duplicate contract id `{}`; first declared at {}:{line}",
                    contract.id,
                    path.display()
                ),
            });
        }
    }
    diagnostics
}

/// Parse one file's contracts, choosing directory or declaration scope by name.
fn read_contracts(path: &Path, root: &Path) -> Result<(Vec<Contract>, Vec<Diagnostic>)> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    ensure!(
        relative.to_str().is_some(),
        "path is not valid UTF-8: {}",
        relative.display()
    );
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        // One unreadable file is a finding: it must not hide every other contract.
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            return Ok((
                Vec::new(),
                vec![Diagnostic {
                    path: relative.into(),
                    line: 1,
                    message: "file is not valid UTF-8".into(),
                }],
            ))
        }
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
    };
    Ok(parse_text(&text, relative))
}

/// Parse contract text for a repository-relative path, choosing directory or
/// declaration scope by file name.
fn parse_text(text: &str, relative: &Path) -> (Vec<Contract>, Vec<Diagnostic>) {
    if relative.file_name().is_some_and(|name| name == "CONTRACTS") {
        parse(text, relative)
    } else {
        attach(text, relative)
    }
}

/// What makes two contracts the same one across revisions: the file, the
/// declaration it is attached to (none for a directory rule), and the ID.
fn identity(contract: &Contract) -> (PathBuf, Option<String>, String) {
    let name = match &contract.scope {
        Scope::Declaration { name, .. } => Some(name.clone()),
        Scope::Directory => None,
    };
    (contract.path.clone(), name, contract.id.clone())
}

/// Run git in `root` and return stdout; the first non-empty stderr line is the error.
fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        // Even `git diff` can refresh .git/index when only file stats changed.
        .args(["-c", "diff.autoRefreshIndex=false"])
        .args(args)
        .current_dir(root)
        .output()
        .context("cannot run git; is git on PATH?")?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("no output")
            .trim()
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A contract present in both revisions whose obligation or metadata differs.
#[derive(Debug, Serialize)]
pub struct Change {
    pub before: Contract,
    pub after: Contract,
}

#[derive(Debug, Default, Serialize)]
pub struct DiffReport {
    pub added: Vec<Contract>,
    pub changed: Vec<Change>,
    pub removed: Vec<Contract>,
}

/**
@cc [owner:Yassimba] contracts-diff-by-identity
A contract diff pairs contracts across revisions by file, attached declaration, and ID, so a reworded or deleted obligation is reported against the same identity it had before, and a renamed declaration reports its contracts as removed and added.
*/
pub fn diff(base: &str) -> Result<DiffReport> {
    let root = cwd_root()?;
    let mut report = DiffReport::default();
    for line in git(&root, &["diff", "--name-status", "--no-renames", base])?.lines() {
        let Some((kind, file)) = line.split_once('\t') else {
            continue;
        };
        let relative = Path::new(file);
        if !contract_file(relative) {
            continue;
        }
        let before = if kind.starts_with('A') {
            String::new()
        } else {
            git(&root, &["show", &format!("{base}:{file}")])?
        };
        let after = if kind.starts_with('D') {
            String::new()
        } else {
            let bytes = fs::read(root.join(file))
                .with_context(|| format!("cannot read {}", relative.display()))?;
            String::from_utf8_lossy(&bytes).into_owned()
        };
        let mut after: BTreeMap<_, _> = parse_text(&after, relative)
            .0
            .into_iter()
            .map(|contract| (identity(&contract), contract))
            .collect();
        for before in parse_text(&before, relative).0 {
            match after.remove(&identity(&before)) {
                Some(after) if after.prose != before.prose || after.metadata != before.metadata => {
                    report.changed.push(Change { before, after });
                }
                Some(_) => {}
                None => report.removed.push(before),
            }
        }
        report.added.extend(after.into_values());
    }
    order(&mut report.added);
    order(&mut report.removed);
    report
        .changed
        .sort_by(|a, b| (&a.after.path, a.after.line).cmp(&(&b.after.path, b.after.line)));
    Ok(report)
}

/// Directory contracts first, then repository-relative path, then line.
fn order(contracts: &mut [Contract]) {
    contracts.sort_by(|a, b| {
        (!matches!(a.scope, Scope::Directory), &a.path, a.line, &a.id).cmp(&(
            !matches!(b.scope, Scope::Directory),
            &b.path,
            b.line,
            &b.id,
        ))
    });
}

/// Every `CONTRACTS` file from the repository root down to `directory`.
fn ancestor_files(directory: &Path, root: &Path) -> Vec<PathBuf> {
    directory
        .ancestors()
        .take_while(|ancestor| ancestor.starts_with(root))
        .map(|ancestor| ancestor.join("CONTRACTS"))
        .filter(|path| path.is_file())
        .collect()
}

/**
@cc [owner:Yassimba] contracts-governing-includes-ancestors
Contracts governing a location include every applicable ancestor CONTRACTS file up to the repository root together with the declarations enclosing that line.
*/
pub fn governing(path: &Path, line: usize) -> Result<Vec<Contract>> {
    governing_ranges(path, &[(line, line)])
}

/// The contracts governing any line in `ranges`, used by `at` for one line and
/// by `affected` for a diff's hunks.
fn governing_ranges(path: &Path, ranges: &[(usize, usize)]) -> Result<Vec<Contract>> {
    let selected = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    ensure!(selected.is_file(), "not a file: {}", path.display());
    let directory = selected.parent().expect("file has a parent");
    let root = crate::project_root(directory);
    let mut contracts = Vec::new();
    for ancestor in ancestor_files(directory, &root) {
        contracts.extend(read_contracts(&ancestor, &root)?.0);
    }
    if supported_source(&selected) {
        contracts.extend(
            read_contracts(&selected, &root)?
                .0
                .into_iter()
                .filter(|contract| match contract.scope {
                    Scope::Declaration { lines, .. } => ranges
                        .iter()
                        .any(|(first, last)| lines.0 <= *last && *first <= lines.1),
                    Scope::Directory => false,
                }),
        );
    }
    order(&mut contracts);
    Ok(contracts)
}

/// The language server for a file, as `(program, arguments, LSP language id)`.
/// Servers are resolved from PATH only; Loom never installs one.
fn language_server(
    extension: &str,
) -> Option<(&'static str, &'static [&'static str], &'static str)> {
    match extension {
        "rs" => Some(("rust-analyzer", &[], "rust")),
        "py" | "pyi" => Some(("ty", &["server"], "python")),
        "ts" | "mts" | "cts" | "tsx" => Some((
            "typescript-language-server",
            &["--stdio"],
            if extension == "tsx" {
                "typescriptreact"
            } else {
                "typescript"
            },
        )),
        _ => None,
    }
}

/// The column of the name a declaration introduces, so a server is asked about
/// the symbol rather than the keyword in front of it.
///
/// ponytail: first identifier after a declaration keyword. Upgrade to a parse
/// tree alongside `governed` if the heuristic starts missing symbols.
fn name_column(line: &str) -> usize {
    let keywords = [
        "fn ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "trait ",
        "function ",
        "const ",
        "let ",
        "type ",
        "interface ",
    ];
    keywords
        .iter()
        .filter_map(|keyword| line.find(keyword).map(|at| at + keyword.len()))
        .min()
        .unwrap_or_else(|| indentation(line))
}

/// Where a contracted declaration is used, and the contracts that govern it.
#[derive(Debug, Serialize)]
pub struct RelatedReport {
    pub contracts: Vec<Contract>,
    pub related: Vec<crate::lsp::Location>,
    pub server: String,
    /// True when `callers` was answered with plain references instead.
    pub fell_back: bool,
}

/// Code that depends on the declaration at a location, via its language server.
pub fn related(path: &Path, line: usize, callers: bool) -> Result<RelatedReport> {
    let selected = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    ensure!(selected.is_file(), "not a file: {}", path.display());
    let extension = selected
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let (program, arguments, language) = language_server(extension)
        .with_context(|| format!("no language server is configured for .{extension} files"))?;
    use crate::system::System as _;
    ensure!(
        crate::system::RealSystem::default().command_exists(program),
        "{program} is not on PATH; install it to use `loom contracts related`"
    );
    let root = crate::project_root(selected.parent().expect("file has a parent"));
    let relative = selected
        .strip_prefix(&root)
        .unwrap_or(&selected)
        .to_path_buf();
    let text = fs::read_to_string(&selected)
        .with_context(|| format!("cannot read {}", selected.display()))?;
    let target = text
        .lines()
        .nth(line - 1)
        .with_context(|| format!("{} has no line {line}", relative.display()))?;
    // Read the obligations before spending a server budget on the query.
    let contracts = governing(path, line)?;
    let (related, fell_back) = crate::lsp::query(
        program,
        arguments,
        &crate::lsp::Query {
            root: &root,
            file: &relative,
            text: &text,
            language,
            line,
            character: name_column(target),
            callers,
        },
    )?;
    Ok(RelatedReport {
        contracts,
        related,
        server: program.to_owned(),
        fell_back,
    })
}

/// Changed line ranges per file in `git diff -U0 <base>`, keyed by the path
/// after the change. A hunk with no added lines is recorded at its position,
/// so a deletion still reports the contract it removed code from.
fn changed_ranges(diff: &str) -> BTreeMap<PathBuf, Vec<(usize, usize)>> {
    let mut ranges: BTreeMap<PathBuf, Vec<(usize, usize)>> = BTreeMap::new();
    let mut file = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            file = Some(PathBuf::from(path));
        } else if let Some(header) = line.strip_prefix("@@ ") {
            let Some(added) = header
                .split_whitespace()
                .find_map(|part| part.strip_prefix('+'))
            else {
                continue;
            };
            let (start, count) = added.split_once(',').unwrap_or((added, "1"));
            let (Ok(start), Ok(count)) = (start.parse::<usize>(), count.parse::<usize>()) else {
                continue;
            };
            if let Some(file) = &file {
                ranges
                    .entry(file.clone())
                    .or_default()
                    .push((start.max(1), start.max(1) + count.saturating_sub(1)));
            }
        }
    }
    ranges
}

/**
@cc [owner:Yassimba] contracts-affected-are-candidates
Contracts reported for a diff are candidate obligations selected by line-range intersection; the command never asserts that a contract is satisfied or violated.
*/
pub fn affected(base: &str, stale: bool) -> Result<Vec<Contract>> {
    let root = cwd_root()?;
    let hunks = git(&root, &["diff", "-U0", base])?;
    let mut contracts = Vec::new();
    for (file, ranges) in changed_ranges(&hunks) {
        let path = root.join(&file);
        // A deleted file has no contracts left to judge.
        if path.is_file() {
            contracts.extend(governing_ranges(&path, &ranges)?);
        }
    }
    order(&mut contracts);
    // Ancestor CONTRACTS repeat for every changed file under them.
    contracts.dedup_by(|a, b| (&a.path, a.line) == (&b.path, b.line));
    if stale {
        // Code under the contract moved, the contract did not: the reviewer must
        // confirm the prose still holds. Directory rules are never expected to
        // move with every change, so only attached contracts can be stale.
        let touched: BTreeSet<_> = {
            let report = diff(base)?;
            report
                .added
                .iter()
                .chain(report.changed.iter().map(|change| &change.after))
                .map(identity)
                .collect()
        };
        contracts.retain(|contract| {
            contract.scope != Scope::Directory && !touched.contains(&identity(contract))
        });
    }
    Ok(contracts)
}

/// A changed declaration that carries no contract of its own.
#[derive(Debug, Serialize)]
pub struct Proposal {
    pub path: PathBuf,
    pub line: usize,
    pub name: String,
    /// Contracts that already govern it: directory rules and enclosing declarations.
    pub governed_by: Vec<Contract>,
}

/**
@cc [owner:Yassimba] contracts-propose-innermost-declaration
For every changed line, propose the innermost declaration with a body that contains it, once per declaration, unless a contract is attached to that declaration; the sweep never writes prose.
*/
pub fn propose(base: &str) -> Result<Vec<Proposal>> {
    let root = cwd_root()?;
    let hunks = git(&root, &["diff", "-U0", base])?;
    let mut proposals = Vec::new();
    for (file, ranges) in changed_ranges(&hunks) {
        let path = root.join(&file);
        if !path.is_file() || !supported_source(&path) {
            continue;
        }
        let Some(style) = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(comment_style)
        else {
            continue;
        };
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let lines: Vec<_> = text.lines().collect();
        let comments = comment_lines(&text, &style);
        // Declarations with a body, innermost last so `rev` finds it first.
        let declarations: Vec<_> = (0..lines.len())
            .filter(|&at| is_declaration(&lines, &comments, at) && is_named(lines[at]))
            .map(|at| governed(&lines, &comments, at))
            .filter(|(_, first, last)| last > first)
            .collect();
        let contracted: BTreeSet<_> = read_contracts(&path, &root)?
            .0
            .into_iter()
            .filter_map(|contract| match contract.scope {
                Scope::Declaration { lines, .. } => Some(lines.0),
                Scope::Directory => None,
            })
            .collect();
        let mut seen = BTreeSet::new();
        for line in ranges.iter().flat_map(|&(first, last)| first..=last) {
            let Some((name, first, last)) = declarations
                .iter()
                .rev()
                .find(|(_, first, last)| (*first..=*last).contains(&line))
            else {
                continue;
            };
            if contracted.contains(first) || !seen.insert(*first) {
                continue;
            }
            proposals.push(Proposal {
                path: file.clone(),
                line: *first,
                name: name.clone(),
                governed_by: governing_ranges(&path, &[(*first, *last)])?,
            });
        }
    }
    proposals.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    Ok(proposals)
}

/// Does the line open a named declaration: `fn`, `class`, `def`, and kin?
///
/// ponytail: keyword list, so unprefixed TypeScript methods are missed.
/// Upgrade with `governed` if tree-sitter ever lands.
fn is_named(line: &str) -> bool {
    const PREFIXES: &[&str] = &["pub", "async", "unsafe", "export", "default", "abstract"];
    const KEYWORDS: &[&str] = &[
        "fn",
        "struct",
        "enum",
        "impl",
        "trait",
        "mod",
        "def",
        "class",
        "function",
        "interface",
        "type",
    ];
    line.split_whitespace()
        .map(|word| word.split('(').next().unwrap_or(word))
        .find(|word| !PREFIXES.contains(word))
        .is_some_and(|word| KEYWORDS.contains(&word))
}

fn identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Does `name` appear as a whole identifier on a line that is not a directive?
fn mentions(text: &str, name: &str) -> bool {
    text.lines()
        .filter(|line| !line.contains("@cc"))
        .any(|line| {
            line.match_indices(name).any(|(at, _)| {
                !line[..at].chars().next_back().is_some_and(identifier_char)
                    && !line[at + name.len()..]
                        .chars()
                        .next()
                        .is_some_and(identifier_char)
            })
        })
}

/**
@cc [owner:Yassimba] contracts-test-anchor-exists
Every value of a contract's `test` metadata names an identifier defined somewhere in the repository's supported source files; a value nothing mentions is a finding.
*/
fn missing_test_anchors(contracts: &[Contract], root: &Path) -> Result<Vec<Diagnostic>> {
    let anchors: Vec<_> = contracts
        .iter()
        .flat_map(|contract| {
            contract
                .metadata
                .iter()
                .filter(|(key, _)| key == "test")
                .flat_map(move |(_, values)| values.iter().map(move |value| (contract, value)))
        })
        .collect();
    if anchors.is_empty() {
        return Ok(Vec::new());
    }
    let mut files = BTreeSet::new();
    collect(root, &mut files)?;
    let texts: Vec<String> = files
        .iter()
        .filter_map(|file| fs::read(file).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .collect();
    Ok(anchors
        .into_iter()
        .filter(|(_, name)| !texts.iter().any(|text| mentions(text, name)))
        .map(|(contract, name)| Diagnostic {
            path: contract.path.clone(),
            line: contract.line,
            message: format!("test `{name}` is not defined anywhere in the repository"),
        })
        .collect())
}

/// Contracts in a file, in a directory tree, or carrying one identifier.
pub fn list(target: &str) -> Result<ListReport> {
    let path = Path::new(target);
    // A target that names a file or directory must exist; only bare words are IDs.
    let by_id = !path.exists()
        && path.parent() == Some(Path::new(""))
        && !supported_source(path)
        && target != "CONTRACTS";
    let selected = if by_id {
        cwd_root()?
    } else {
        path.canonicalize()
            .with_context(|| format!("cannot resolve {}", path.display()))?
    };
    let root = crate::project_root(if selected.is_dir() {
        &selected
    } else {
        selected.parent().expect("file has a parent")
    });
    let mut files = BTreeSet::new();
    if selected.is_dir() {
        collect(&selected, &mut files)?;
    } else {
        ensure!(
            contract_file(&selected),
            "expected a CONTRACTS file or a supported source file: {}",
            path.display()
        );
        files.insert(selected);
    }
    let mut contracts = Vec::new();
    for file in files {
        contracts.extend(read_contracts(&file, &root)?.0);
    }
    if by_id {
        contracts.retain(|contract| contract.id == target);
    }
    order(&mut contracts);
    Ok(ListReport { contracts, by_id })
}

/**
@cc [owner:Yassimba] contracts-check-read-only
Checking contracts reads the selected filesystem scope and produces diagnostics without creating, modifying, or deleting files.
*/
/**
@cc [owner:Yassimba] contracts-deterministic-findings
Contract files and diagnostics are reported in repository-relative path and source-line order, independent of filesystem traversal order.
*/
pub fn check(path: &Path) -> Result<CheckReport> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("cannot inspect {}", path.display()))?;
    ensure!(
        !metadata.is_symlink(),
        "selected path is a symlink: {}",
        path.display()
    );
    ensure!(
        metadata.is_dir() || metadata.is_file(),
        "not a regular file or directory: {}",
        path.display()
    );
    if metadata.is_file() {
        ensure!(
            contract_file(path),
            "expected a CONTRACTS file or a supported source file: {}",
            path.display()
        );
    }
    let selected = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    let directory = if metadata.is_dir() {
        selected.as_path()
    } else {
        selected.parent().expect("file has a parent")
    };
    let root = crate::project_root(directory);
    let mut files = BTreeSet::new();
    if metadata.is_dir() {
        collect(&selected, &mut files)?;
    } else {
        files.insert(selected.clone());
    }
    files.extend(ancestor_files(directory, &root));
    let mut contracts = Vec::new();
    let mut report = CheckReport {
        checked: files.len(),
        diagnostics: Vec::new(),
    };
    for path in files {
        let (parsed, diagnostics) = read_contracts(&path, &root)?;
        contracts.extend(parsed);
        report.diagnostics.extend(diagnostics);
    }
    report.diagnostics.extend(duplicate_diagnostics(&contracts));
    report
        .diagnostics
        .extend(missing_test_anchors(&contracts, &root)?);
    report
        .diagnostics
        .sort_by(|a, b| (&a.path, a.line, &a.message).cmp(&(&b.path, b.line, &b.message)));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_declaration_starts_with_a_keyword_after_modifiers() {
        assert!(is_named("pub(crate) async fn go() {"));
        assert!(is_named("    def go(self):"));
        assert!(is_named("export default class Go {"));
        assert!(!is_named("let resources = resources"));
        assert!(!is_named("}"));
    }

    #[test]
    fn a_test_anchor_is_a_whole_identifier_outside_directives() {
        assert!(mentions("fn pays_once() {}", "pays_once"));
        assert!(!mentions("fn pays_once_more() {}", "pays_once"));
        assert!(!mentions("/// @cc [test:pays_once] id", "pays_once"));
    }

    #[test]
    fn a_diff_becomes_one_line_range_per_hunk() {
        let diff = "diff --git a/pkg/pay.py b/pkg/pay.py\n\
--- a/pkg/pay.py\n\
+++ b/pkg/pay.py\n\
@@ -6 +6 @@ def charge(amount):\n\
-    return amount\n\
+    return amount * 2\n\
@@ -20,0 +21,3 @@ def other():\n\
+    added\n\
+    added\n\
+    added\n\
--- a/gone.rs\n\
+++ /dev/null\n";
        let ranges = changed_ranges(diff);
        assert_eq!(
            ranges[Path::new("pkg/pay.py")],
            vec![(6, 6), (21, 23)],
            "{ranges:?}"
        );
        // A pure deletion has no `+++ b/` path, so nothing is attributed to it.
        assert_eq!(ranges.len(), 1, "{ranges:?}");
    }
}
