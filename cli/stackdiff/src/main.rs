use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, CommandFactory, Parser, ValueEnum};

use rayon::prelude::*;
use stackdiff::boxes::{render_boxes, BoxOptions, Direction};
use stackdiff::calltree::{build_call_tree, build_caller_tree, reverse_index};
use stackdiff::diff::{dedupe_subtrees, prune_unchanged};
use stackdiff::extract::{build_index, extract_functions, FunctionIndex};
use stackdiff::git::{
    assert_git_repo, list_source_files, read_snapshot_files, resolve_snapshots, verify_commit,
};
use stackdiff::infer::{diff_entry, infer_entries};
use stackdiff::noise::{report as noise_report, scrub_index, NoiseFilter};
use stackdiff::render::{diff_stat, render_diff, render_mermaid, RenderOptions};
use stackdiff::types::{DiffNode, DiffStatus, Snapshot};
use stackdiff::views::{
    class_mermaid, lineage_mermaid, module_mermaid, render_colored, render_colored_flip,
    sequence_mermaid, Lineage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Dir {
    Lr,
    Td,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum View {
    /// Rich text tree (default)
    Text,
    /// Boxed graph (-m)
    Boxes,
    /// Sequence diagram: lifelines per file
    Seq,
    /// Class/ER diagram of the types in play
    Er,
    /// One node per file, cross-file call arrows
    Modules,
    /// Call DAG at data granularity: each function drawn once, fan-in
    /// visible, data objects as stadium nodes, bindings on the edges
    Lineage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// ANSI tree for terminals
    Text,
    /// Full node data (location, doc, returns, binding, args, consumes)
    Json,
    /// The tree inside a ```diff fence — paste into chat/PRs for coloring
    Markdown,
    /// A Mermaid flowchart (added green, removed red) in a ```mermaid fence
    Mermaid,
}

/// Diff call stacks across git commits — like `git diff`, but for who-calls-whom.
///
/// Semantics (like git diff):
///   (no refs)      from=HEAD, to=working tree
///   <from>         from=<from>, to=working tree
///   <from> <to>    compare those two trees
#[derive(Debug, Parser)]
#[command(name = "stackdiff", version, about, verbatim_doc_comment)]
struct Cli {
    /// Left / "before" ref (positional, git-diff style)
    #[arg(help_heading = "Range")]
    from_ref: Option<String>,

    /// Right / "after" ref
    #[arg(help_heading = "Range")]
    to_ref: Option<String>,

    /// Left / "before" ref (flag form; overrides the positional)
    #[arg(long, help_heading = "Range")]
    from: Option<String>,

    /// Right / "after" ref (flag form; overrides the positional)
    #[arg(long, help_heading = "Range")]
    to: Option<String>,

    /// Entrypoint(s): functionName or ClassName.method.
    /// If omitted, infer exported functions whose call trees changed.
    #[arg(short, long = "entry", action = ArgAction::Append, help_heading = "Scope")]
    entries: Vec<String>,

    /// Print one world's call tree instead of a diff.
    /// The world is <from> if given, otherwise the working tree.
    /// Without --entry, lists the exported entrypoints of that world.
    #[arg(long, help_heading = "Range")]
    tree: bool,

    /// Max call-tree depth [default: 3, or .stackdiff.toml]
    #[arg(long, help_heading = "Scope")]
    max_depth: Option<usize>,

    /// Include files that .gitignore excludes
    #[arg(long, help_heading = "Scope")]
    no_ignore: bool,

    /// Include test files (excluded by default; a test path after -- also
    /// brings them in)
    #[arg(long, help_heading = "Scope")]
    tests: bool,

    /// Show builtin/plumbing calls that hide by default (len, clone, …)
    #[arg(long, help_heading = "Noise")]
    noise: bool,

    /// Extra hide globs on top of the defaults and .stackdiff.toml
    #[arg(long, action = ArgAction::Append, help_heading = "Noise")]
    hide: Vec<String>,

    /// Print the unresolved-call frequency table (what hides, what shows)
    /// and exit — tune .stackdiff.toml from it
    #[arg(long, help_heading = "Noise")]
    noise_report: bool,

    /// Reverse graph: who calls the entry, and who calls the callers
    #[arg(long, help_heading = "Scope")]
    callers: bool,

    /// Module zoom: one node per file, arrows where calls cross files
    #[arg(long, hide = true)]
    modules: bool,

    /// How to draw the graph [default: text, or .stackdiff.toml]
    #[arg(long, value_enum, help_heading = "View")]
    view: Option<View>,

    /// Labels only: hide binding/args/returns, locations, and doc lines
    #[arg(long, help_heading = "View")]
    plain: bool,

    /// Show whole trees in diffs instead of pruning unchanged limbs
    #[arg(long, help_heading = "View")]
    full: bool,

    /// Full control-flow granularity: keep if/else arms and unresolved
    /// plumbing calls the lineage default flattens away
    #[arg(long, help_heading = "View")]
    flow: bool,

    /// Unchanged siblings kept around each change [default: 1]
    #[arg(long, help_heading = "View")]
    context: Option<usize>,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Text, help_heading = "Output")]
    format: Format,

    /// Shorthand for --view lineage: the connected call DAG
    #[arg(short = 'm', long, help_heading = "View")]
    boxes: bool,

    /// Box-graph direction: lr grows down the terminal, td grows across
    #[arg(long, value_enum, help_heading = "View")]
    dir: Option<Dir>,

    /// Shorthand for --view seq
    #[arg(long, hide = true)]
    seq: bool,

    /// Shorthand for --view er
    #[arg(long, hide = true)]
    er: bool,

    /// Append a +added/-removed summary per entry (diff mode)
    #[arg(long, help_heading = "Output")]
    stat: bool,

    /// Re-expand repeated subtrees instead of "▸ shown above" stubs
    #[arg(long, help_heading = "Output")]
    no_dedupe: bool,

    /// Make locations clickable: an editor name (zed, vscode, cursor, file,
    /// none) or a URL template with {path} and {line}.
    /// Defaults to $STACKDIFF_EDITOR, then the host terminal.
    #[arg(long, help_heading = "Output")]
    link: Option<String>,

    /// Generate shell completions (bash, zsh, fish, elvish, powershell)
    #[arg(long, value_name = "SHELL", help_heading = "Output")]
    completions: Option<clap_complete::Shell>,

    /// When to color output
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, help_heading = "Output")]
    color: ColorChoice,

    /// Repository to operate on (defaults to the current directory)
    #[arg(long, short = 'C')]
    repo: Option<PathBuf>,

    /// Limit to these paths
    #[arg(last = true)]
    paths: Vec<String>,
}

/// Resolve the hyperlink template: --link, then $STACKDIFF_EDITOR, then the
/// terminal's own editor (Zed/VS Code terminals), then file://. "none" turns
/// links off.
impl Cli {
    fn depth(&self) -> usize {
        self.max_depth.unwrap_or(3)
    }

    fn ctx(&self) -> usize {
        self.context.unwrap_or(1)
    }

    fn the_view(&self) -> View {
        if let Some(view) = self.view {
            return view;
        }
        if self.boxes {
            View::Lineage
        } else if self.seq {
            View::Seq
        } else if self.er {
            View::Er
        } else if self.modules {
            View::Modules
        } else {
            View::Text
        }
    }

    fn direction(&self) -> Dir {
        self.dir.unwrap_or(Dir::Lr)
    }

    /// Fill unset flags from .stackdiff.toml [defaults].
    fn absorb(&mut self, defaults: &stackdiff::noise::Defaults) {
        self.max_depth = self.max_depth.or(defaults.max_depth);
        self.context = self.context.or(defaults.context);
        self.link = self.link.clone().or_else(|| defaults.link.clone());
        self.tests |= defaults.tests;
        if self.view.is_none() && !self.boxes && !self.seq && !self.er && !self.modules {
            self.view = match defaults.view.as_deref() {
                Some("lineage") => Some(View::Lineage),
                Some("boxes") => Some(View::Boxes),
                Some("seq") => Some(View::Seq),
                Some("er") => Some(View::Er),
                Some("modules") => Some(View::Modules),
                _ => None,
            };
        }
        if self.dir.is_none() {
            self.dir = match defaults.dir.as_deref() {
                Some("td") => Some(Dir::Td),
                Some("lr") => Some(Dir::Lr),
                _ => None,
            };
        }
    }
}

fn link_template(cli: &Cli) -> Option<String> {
    let choice = cli
        .link
        .clone()
        .or_else(|| std::env::var("STACKDIFF_EDITOR").ok())
        .or_else(|| match std::env::var("TERM_PROGRAM").ok()?.as_str() {
            "zed" => Some("zed".to_string()),
            "vscode" => Some("vscode".to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "file".to_string());
    match choice.as_str() {
        "none" => None,
        "zed" => Some("zed://file/{path}:{line}".to_string()),
        "vscode" => Some("vscode://file/{path}:{line}".to_string()),
        "cursor" => Some("cursor://file/{path}:{line}".to_string()),
        "file" => Some("file://{path}".to_string()),
        template => Some(template.to_string()),
    }
}

fn render_options(cli: &Cli, cwd: &Path, color: bool) -> RenderOptions {
    RenderOptions {
        color,
        rich: !cli.plain,
        // OSC 8 escapes only make sense on a terminal.
        link: if cli.format == Format::Text {
            link_template(cli)
        } else {
            None
        },
        repo_root: Some(cwd.to_path_buf()),
    }
}

fn load_index(
    cwd: &Path,
    snapshot: &Snapshot,
    cli: &Cli,
    filter: &NoiseFilter,
) -> Result<FunctionIndex> {
    let (path_filters, no_ignore, include_tests) = (&cli.paths, cli.no_ignore, cli.tests);
    let files = list_source_files(cwd, snapshot, path_filters, no_ignore, include_tests)?;

    let sources = read_snapshot_files(cwd, snapshot, &files);
    let all: Vec<_> = sources
        .par_iter()
        .flat_map(|(file, source)| {
            let Some(source) = source else {
                return Vec::new();
            };
            match extract_functions(file, source) {
                Ok(functions) => functions,
                Err(error) => {
                    eprintln!(
                        "warn: failed to parse {file} @ {}: {error}",
                        snapshot.describe()
                    );
                    Vec::new()
                }
            }
        })
        .collect();
    let mut index = build_index(all);
    scrub_index(&mut index, filter);
    Ok(index)
}

fn tree_as_diff(node: &stackdiff::types::CallNode) -> DiffNode {
    DiffNode {
        key: node.key.clone(),
        label: node.label.clone(),
        kind: node.kind,
        status: DiffStatus::Same,
        location: node.location.clone(),
        doc: node.doc.clone(),
        returns: node.returns.clone(),
        signature: node.signature.clone(),
        meta: node.meta.clone(),
        children: node.children.iter().map(tree_as_diff).collect(),
    }
}

fn term_width() -> Option<usize> {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .or_else(|| std::env::var("COLUMNS").ok()?.parse().ok())
}

/// Field definitions for --er, from the worktree (or --from ref) sources.
fn load_types(cli: &Cli) -> std::collections::BTreeMap<String, Vec<(String, Option<String>)>> {
    let mut types = std::collections::BTreeMap::new();
    let cwd = cli
        .repo
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("no current directory"));
    let snapshot = Snapshot::Worktree;
    let Ok(files) = list_source_files(&cwd, &snapshot, &cli.paths, cli.no_ignore, cli.tests) else {
        return types;
    };
    for (file, source) in read_snapshot_files(&cwd, &snapshot, &files) {
        let Some(source) = source else { continue };
        if let Ok(found) = stackdiff::extract::extract_types(&file, &source) {
            for info in found {
                types.entry(info.name).or_insert(info.fields);
            }
        }
    }
    types
}

fn print_trees(cli: &Cli, header: &str, trees: &[DiffNode], options: &RenderOptions) {
    // The default tree reads at lineage granularity; --flow restores
    // branches and plumbing.
    let lineage_cut: Vec<DiffNode>;
    let trees: &[DiffNode] = if !cli.flow
        && matches!(cli.the_view(), View::Text | View::Boxes)
        && cli.format != Format::Json
    {
        lineage_cut = trees.iter().map(stackdiff::diff::lineage_prune).collect();
        &lineage_cut
    } else {
        trees
    };
    if cli.the_view() == View::Seq {
        println!("{header}\n");
        for (index, tree) in trees.iter().enumerate() {
            if index > 0 {
                println!();
            }
            let (source, marks) = sequence_mermaid(tree);
            match render_colored(&source, &marks, options.color, term_width()) {
                Ok(diagram) => println!("{diagram}"),
                Err(error) => eprintln!("--seq failed for {}: {error}", tree.key),
            }
            stat_line(cli, tree);
        }
        return;
    }
    if cli.the_view() == View::Lineage {
        println!("{header}\n");
        show_lineage(cli, trees, options, None);
        return;
    }
    if cli.the_view() == View::Modules {
        println!("{header}\n");
        match module_mermaid(trees) {
            Some((source, marks)) => {
                match render_colored(&source, &marks, options.color, term_width()) {
                    Ok(diagram) => println!("{diagram}"),
                    Err(error) => eprintln!("--modules failed: {error}"),
                }
            }
            None => println!("No cross-file calls in these graphs — nothing to draw."),
        }
        return;
    }
    if cli.the_view() == View::Er {
        println!("{header}\n");
        match class_mermaid(trees, &load_types(cli)) {
            Some((source, marks)) => {
                match render_colored(&source, &marks, options.color, term_width()) {
                    Ok(diagram) => println!("{diagram}"),
                    Err(error) => eprintln!("--er failed: {error}"),
                }
            }
            None => println!("No Type.method calls in these graphs — nothing to draw."),
        }
        return;
    }
    if cli.the_view() == View::Boxes {
        let box_options = BoxOptions {
            color: options.color,
            dir: match cli.direction() {
                Dir::Lr => Direction::LeftRight,
                Dir::Td => Direction::TopDown,
            },
            link: link_template(cli),
            repo_root: options.repo_root.clone(),
            max_width: term_width(),
        };
        println!("{header}\n");
        for (index, tree) in trees.iter().enumerate() {
            if index > 0 {
                println!();
            }
            println!("{}", render_boxes(tree, &box_options));
            stat_line(cli, tree);
        }
        return;
    }
    match cli.format {
        Format::Json => {
            let value = serde_json::json!({ "header": header, "entries": trees });
            println!("{}", serde_json::to_string_pretty(&value).expect("json"));
        }
        Format::Mermaid => {
            println!("{header}\n");
            for tree in trees {
                println!("```mermaid");
                println!("{}", render_mermaid(tree));
                println!("```");
            }
        }
        Format::Markdown => {
            println!("{header}\n");
            println!("```diff");
            for (index, tree) in trees.iter().enumerate() {
                if index > 0 {
                    println!();
                }
                println!(
                    "{}",
                    render_diff(
                        tree,
                        &RenderOptions {
                            color: false,
                            link: None,
                            ..options.clone()
                        }
                    )
                );
                stat_line(cli, tree);
            }
            println!("```");
        }
        Format::Text => {
            println!("{header}\n");
            for (index, tree) in trees.iter().enumerate() {
                if index > 0 {
                    println!();
                }
                println!("{}", render_diff(tree, options));
                stat_line(cli, tree);
            }
        }
    }
}

type Rebuild<'a> = &'a dyn Fn(&str) -> Vec<DiffNode>;

/// Render the lineage view; oversized graphs become a cluster list, and —
/// on a terminal, when `rebuild` can produce a single entry's trees — an
/// interactive picker that drills into the chosen cluster.
fn show_lineage(cli: &Cli, trees: &[DiffNode], options: &RenderOptions, rebuild: Option<Rebuild>) {
    let template = link_template(cli);
    let link = template
        .as_deref()
        .map(|template| (template, options.repo_root.as_deref()));
    let dir = match cli.dir {
        Some(Dir::Td) => "TD",
        _ => "LR",
    };
    let limit = (!cli.full).then_some(60);
    match lineage_mermaid(trees, link, dir, limit) {
        Some(Lineage::Graph(source, marks)) => {
            match render_colored_flip(
                &source,
                &marks,
                options.color,
                term_width(),
                cli.dir.is_none(),
            ) {
                Ok(diagram) => println!("{diagram}"),
                Err(error) => eprintln!("lineage view failed: {error}"),
            }
        }
        Some(Lineage::Overview(rows)) => {
            let interactive = std::io::stdout().is_terminal() && rebuild.is_some();
            if !interactive {
                println!("Graph too big to draw — {} clusters. Open one:", rows.len());
                for row in rows.iter().take(20) {
                    println!(
                        "  {:>4} changed / {:>4} nodes   stackdiff … -e {} -m",
                        row.changed, row.size, row.entry
                    );
                }
                if rows.len() > 20 {
                    println!("  … {} more clusters", rows.len() - 20);
                }
                println!("(--full draws it anyway)");
                return;
            }
            let rebuild = rebuild.expect("interactive requires rebuild");
            let items: Vec<String> = rows
                .iter()
                .take(30)
                .map(|row| {
                    format!(
                        "{:>4} changed / {:>4} nodes   {}",
                        row.changed, row.size, row.entry
                    )
                })
                .collect();
            loop {
                let picked = dialoguer::Select::new()
                    .with_prompt(format!(
                        "Graph too big to draw — {} clusters. Open one (Esc quits)",
                        rows.len()
                    ))
                    .items(&items)
                    .default(0)
                    .interact_opt()
                    .ok()
                    .flatten();
                let Some(index) = picked else { break };
                let entry_trees = rebuild(&rows[index].entry);
                if entry_trees.is_empty() {
                    eprintln!("Could not rebuild {}", rows[index].entry);
                    continue;
                }
                println!();
                match lineage_mermaid(&entry_trees, link, dir, None) {
                    Some(Lineage::Graph(source, marks)) => match render_colored_flip(
                        &source,
                        &marks,
                        options.color,
                        term_width(),
                        cli.dir.is_none(),
                    ) {
                        Ok(diagram) => println!("{diagram}\n"),
                        Err(error) => eprintln!("lineage view failed: {error}"),
                    },
                    _ => println!("Nothing to draw for {}.", rows[index].entry),
                }
            }
        }
        None => println!("No resolved calls in these graphs — nothing to draw."),
    }
}

fn stat_line(cli: &Cli, tree: &DiffNode) {
    if !cli.stat {
        return;
    }
    let (added, removed, changed) = diff_stat(tree);
    println!("  {}: +{added} -{removed} !{changed}", tree.key);
}

/// Entrypoints grouped by file, optionally filtered.
fn list_entries(index: &FunctionIndex, snapshot: &Snapshot, filter: Option<&str>) {
    let mut by_file: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for (key, info) in index {
        if !info.exported || key.starts_with("new ") {
            continue;
        }
        if let Some(filter) = filter {
            if !key.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        by_file.entry(info.file.as_str()).or_default().push(key);
    }
    let total: usize = by_file.values().map(Vec::len).sum();
    match filter {
        Some(filter) => println!(
            "Entrypoints matching \"{filter}\" @ {} ({total}):",
            snapshot.describe()
        ),
        None => println!("Exported entrypoints @ {} ({total}):", snapshot.describe()),
    }
    for (file, mut keys) in by_file {
        keys.sort_by_key(|k| k.to_lowercase());
        println!("  {file}");
        for key in keys {
            println!("    {key}");
        }
    }
}

/// A missed entry gets suggestions, not a shrug.
fn miss(entry: &str, index: &FunctionIndex) {
    let close = stackdiff::calltree::suggest_entries(entry, index, 5);
    if close.is_empty() {
        eprintln!("Entrypoint not found: {entry}");
    } else {
        eprintln!(
            "Entrypoint not found: {entry} — did you mean: {}?",
            close.join(", ")
        );
    }
}

fn run_tree(cli: &Cli, cwd: &Path, color: bool) -> Result<i32> {
    let reference = cli.from.as_deref().or(cli.from_ref.as_deref());
    let snapshot = match reference {
        Some(reference) => {
            verify_commit(cwd, reference)?;
            Snapshot::Commit(reference.to_string())
        }
        None => Snapshot::Worktree,
    };
    let filter = NoiseFilter::load(cwd, !cli.noise, &cli.hide);
    let index = load_index(cwd, &snapshot, cli, &filter)?;

    if cli.noise_report {
        println!("{}", noise_report(&index, &filter));
        return Ok(0);
    }

    if cli.entries.is_empty() {
        if cli.the_view() == View::Lineage {
            // No entry: start from the program's own doors.
            let roots = stackdiff::calltree::detect_entrypoints(&index, 12);
            if roots.is_empty() {
                println!("No entry points detected — name one with -e.");
                return Ok(0);
            }
            println!(
                "stackdiff -m @ {} · auto roots: {}\n",
                snapshot.describe(),
                roots.join(", ")
            );
            let trees: Vec<DiffNode> = roots
                .iter()
                .map(|root| tree_as_diff(&build_call_tree(root, &index, cli.depth())))
                .collect();
            let options = render_options(cli, cwd, color);
            let rebuild = |entry: &str| -> Vec<DiffNode> {
                stackdiff::calltree::resolve_entry(entry, &index)
                    .map(|key| vec![tree_as_diff(&build_call_tree(&key, &index, cli.depth()))])
                    .unwrap_or_default()
            };
            show_lineage(cli, &trees, &options, Some(&rebuild));
            return Ok(0);
        }
        list_entries(&index, &snapshot, None);
        return Ok(0);
    }

    let reverse = cli.callers.then(|| reverse_index(&index));
    let mut trees = Vec::new();
    for entry in &cli.entries {
        let Some(resolved) = stackdiff::calltree::resolve_entry(entry, &index) else {
            miss(entry, &index);
            continue;
        };
        trees.push(tree_as_diff(&match &reverse {
            Some(reverse) => build_caller_tree(&resolved, &index, reverse, cli.depth()),
            None => build_call_tree(&resolved, &index, cli.depth()),
        }));
    }
    if trees.is_empty() {
        return Ok(1);
    }
    let options = render_options(cli, cwd, color);
    print_trees(
        cli,
        &format!("stackdiff --tree @ {}", snapshot.describe()),
        &trees,
        &options,
    );
    Ok(0)
}

fn run_diff(cli: &Cli, cwd: &Path, color: bool) -> Result<i32> {
    let (from, to) = resolve_snapshots(
        cli.from.as_deref().or(cli.from_ref.as_deref()),
        cli.to.as_deref().or(cli.to_ref.as_deref()),
    );
    if let Snapshot::Commit(reference) = &from {
        verify_commit(cwd, reference)?;
    }
    if let Snapshot::Commit(reference) = &to {
        verify_commit(cwd, reference)?;
    }

    let filter = NoiseFilter::load(cwd, !cli.noise, &cli.hide);
    let (before, after) = rayon::join(
        || load_index(cwd, &from, cli, &filter),
        || load_index(cwd, &to, cli, &filter),
    );
    let (before, after) = (before?, after?);

    if cli.noise_report {
        println!("{}", noise_report(&after, &filter));
        return Ok(0);
    }

    if cli.callers {
        if cli.entries.is_empty() {
            anyhow::bail!("--callers needs --entry: whose callers?");
        }
        let before_reverse = reverse_index(&before);
        let after_reverse = reverse_index(&after);
        let mut diffs = Vec::new();
        for entry in &cli.entries {
            let before_key = stackdiff::calltree::resolve_entry(entry, &before);
            let after_key = stackdiff::calltree::resolve_entry(entry, &after);
            let Some(key) = after_key.or(before_key) else {
                eprintln!("Entrypoint not found: {entry}");
                continue;
            };
            let before_tree = build_caller_tree(&key, &before, &before_reverse, cli.depth());
            let after_tree = build_caller_tree(&key, &after, &after_reverse, cli.depth());
            diffs.push(stackdiff::diff::diff_trees(&before_tree, &after_tree));
        }
        if diffs.is_empty() {
            println!("No caller graphs to show.");
            return Ok(0);
        }
        let options = render_options(cli, cwd, color);
        print_trees(
            cli,
            &format!(
                "stackdiff --callers {} → {}",
                from.describe(),
                to.describe()
            ),
            &diffs,
            &options,
        );
        return Ok(0);
    }

    let entries = infer_entries(&before, &after, &cli.entries, cli.depth())?;

    if entries.is_empty() {
        println!(
            "No callstack changes between {} and {}.",
            from.describe(),
            to.describe()
        );
        if to == Snapshot::Worktree && from == Snapshot::Commit("HEAD".to_string()) {
            println!(
                "Tip: with a clean working tree, compare refs — `stackdiff main`, `stackdiff HEAD~1`, or `stackdiff <from> <to>`."
            );
        }
        return Ok(0);
    }

    let mut diffs: Vec<_> = entries
        .par_iter()
        .filter_map(|entry| diff_entry(entry, &before, &after, cli.depth()))
        .map(|diff| {
            if cli.full {
                diff
            } else {
                prune_unchanged(&diff, cli.ctx())
            }
        })
        .collect();

    if diffs.is_empty() {
        println!("No callstack changes for inferred entrypoints.");
        return Ok(0);
    }

    // Biggest change first — the story, not the alphabet.
    diffs.sort_by_key(|diff| {
        let (a, r, c) = diff_stat(diff);
        std::cmp::Reverse(a + r + c)
    });
    if !cli.no_dedupe {
        dedupe_subtrees(&mut diffs);
    }

    let (mut added, mut removed, mut changed) = (0, 0, 0);
    for diff in &diffs {
        let (a, r, c) = diff_stat(diff);
        added += a;
        removed += r;
        changed += c;
    }
    let biggest = &diffs[0];
    let (ba, br, bc) = diff_stat(biggest);
    let header = format!(
        "stackdiff {} → {}\n{} entries changed · +{added} −{removed} !{changed} · biggest: {} (+{ba} −{br} !{bc})",
        from.describe(),
        to.describe(),
        diffs.len(),
        biggest.key,
    );

    let options = render_options(cli, cwd, color);
    if cli.the_view() == View::Lineage {
        println!("{header}\n");
        let rebuild = |entry: &str| -> Vec<DiffNode> {
            diff_entry(entry, &before, &after, cli.depth())
                .map(|diff| vec![diff])
                .unwrap_or_default()
        };
        show_lineage(cli, &diffs, &options, Some(&rebuild));
        return Ok(0);
    }
    print_trees(cli, &header, &diffs, &options);
    Ok(0)
}

fn main() {
    let mut cli = Cli::parse();
    {
        let cwd = cli
            .repo
            .clone()
            .unwrap_or_else(|| std::env::current_dir().expect("no current directory"));
        let config = stackdiff::noise::load_file(&cwd);
        cli.absorb(&config.defaults);
    }
    let cli = cli;

    if let Some(shell) = cli.completions {
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            "stackdiff",
            &mut std::io::stdout(),
        );
        return;
    }

    let color = match cli.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => std::io::stdout().is_terminal(),
    };

    let cwd = cli
        .repo
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("no current directory"));

    let code = (|| -> Result<i32> {
        assert_git_repo(&cwd)?;
        if cli.tree {
            run_tree(&cli, &cwd, color)
        } else {
            run_diff(&cli, &cwd, color)
        }
    })()
    .unwrap_or_else(|error| {
        eprintln!("{error}");
        1
    });

    std::process::exit(code);
}
