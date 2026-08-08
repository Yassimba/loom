use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};

use rayon::prelude::*;
use stackdiff::boxes::render_boxes;
use stackdiff::calltree::build_call_tree;
use stackdiff::diff::prune_unchanged;
use stackdiff::extract::{build_index, extract_functions, FunctionIndex};
use stackdiff::git::{
    assert_git_repo, list_source_files, read_snapshot_files, resolve_snapshots, verify_commit,
};
use stackdiff::infer::{diff_entry, infer_entries};
use stackdiff::render::{diff_stat, render_diff, render_mermaid, RenderOptions};
use stackdiff::types::{DiffNode, DiffStatus, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
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
    from_ref: Option<String>,

    /// Right / "after" ref
    to_ref: Option<String>,

    /// Left / "before" ref (flag form; overrides the positional)
    #[arg(long)]
    from: Option<String>,

    /// Right / "after" ref (flag form; overrides the positional)
    #[arg(long)]
    to: Option<String>,

    /// Entrypoint(s): functionName or ClassName.method.
    /// If omitted, infer exported functions whose call trees changed.
    #[arg(short, long = "entry", action = ArgAction::Append)]
    entries: Vec<String>,

    /// Print one world's call tree instead of a diff.
    /// The world is <from> if given, otherwise the working tree.
    /// Without --entry, lists the exported entrypoints of that world.
    #[arg(long)]
    tree: bool,

    /// Max call-tree depth
    #[arg(long, default_value_t = 3)]
    max_depth: usize,

    /// Include files that .gitignore excludes
    #[arg(long)]
    no_ignore: bool,

    /// Include test files (excluded by default; a test path after -- also
    /// brings them in)
    #[arg(long)]
    tests: bool,

    /// Labels only: hide binding/args/returns, locations, and doc lines
    #[arg(long)]
    plain: bool,

    /// Show whole trees in diffs instead of pruning unchanged limbs
    #[arg(long)]
    full: bool,

    /// Unchanged siblings kept around each change (default 1)
    #[arg(long, default_value_t = 1)]
    context: usize,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Boxed-graph view in the terminal (mermaid-style boxes, colored by
    /// status); shorthand that overrides --format
    #[arg(short = 'm', long)]
    boxes: bool,

    /// Append a +added/-removed summary per entry (diff mode)
    #[arg(long)]
    stat: bool,

    /// Make locations clickable: an editor name (zed, vscode, cursor, file)
    /// or a URL template with {path} and {line}.
    /// Defaults to $STACKDIFF_EDITOR when set.
    #[arg(long)]
    link: Option<String>,

    /// When to color output
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Repository to operate on (defaults to the current directory)
    #[arg(long, short = 'C')]
    repo: Option<PathBuf>,

    /// Limit to these paths
    #[arg(last = true)]
    paths: Vec<String>,
}

fn link_template(cli: &Cli) -> Option<String> {
    let choice = cli
        .link
        .clone()
        .or_else(|| std::env::var("STACKDIFF_EDITOR").ok())?;
    Some(match choice.as_str() {
        "zed" => "zed://file/{path}:{line}".to_string(),
        "vscode" => "vscode://file/{path}:{line}".to_string(),
        "cursor" => "cursor://file/{path}:{line}".to_string(),
        "file" => "file://{path}".to_string(),
        template => template.to_string(),
    })
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
    path_filters: &[String],
    no_ignore: bool,
    include_tests: bool,
) -> Result<FunctionIndex> {
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
    Ok(build_index(all))
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
        meta: node.meta.clone(),
        children: node.children.iter().map(tree_as_diff).collect(),
    }
}

fn print_trees(cli: &Cli, header: &str, trees: &[DiffNode], options: &RenderOptions) {
    if cli.boxes {
        println!("{header}\n");
        for (index, tree) in trees.iter().enumerate() {
            if index > 0 {
                println!();
            }
            println!("{}", render_boxes(tree, options.color));
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

fn stat_line(cli: &Cli, tree: &DiffNode) {
    if !cli.stat {
        return;
    }
    let (added, removed, changed) = diff_stat(tree);
    println!("  {}: +{added} -{removed} !{changed}", tree.key);
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
    let index = load_index(cwd, &snapshot, &cli.paths, cli.no_ignore, cli.tests)?;

    if cli.entries.is_empty() {
        let mut exported: Vec<&String> = index
            .iter()
            .filter(|(key, info)| info.exported && !key.starts_with("new "))
            .map(|(key, _)| key)
            .collect();
        exported.sort_by_key(|key| key.to_lowercase());
        println!("Exported entrypoints @ {}:", snapshot.describe());
        for key in exported {
            println!("  {key}");
        }
        return Ok(0);
    }

    let mut trees = Vec::new();
    for entry in &cli.entries {
        let Some(resolved) = stackdiff::calltree::resolve_entry(entry, &index) else {
            eprintln!("Entrypoint not found: {entry}");
            continue;
        };
        trees.push(tree_as_diff(&build_call_tree(
            &resolved,
            &index,
            cli.max_depth,
        )));
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

    let (before, after) = rayon::join(
        || load_index(cwd, &from, &cli.paths, cli.no_ignore, cli.tests),
        || load_index(cwd, &to, &cli.paths, cli.no_ignore, cli.tests),
    );
    let (before, after) = (before?, after?);

    let entries = infer_entries(&before, &after, &cli.entries, cli.max_depth)?;

    if entries.is_empty() {
        println!(
            "No callstack changes between {} and {}.",
            from.describe(),
            to.describe()
        );
        return Ok(0);
    }

    let diffs: Vec<_> = entries
        .par_iter()
        .filter_map(|entry| diff_entry(entry, &before, &after, cli.max_depth))
        .map(|diff| {
            if cli.full {
                diff
            } else {
                prune_unchanged(&diff, cli.context)
            }
        })
        .collect();

    if diffs.is_empty() {
        println!("No callstack changes for inferred entrypoints.");
        return Ok(0);
    }

    let options = render_options(cli, cwd, color);
    print_trees(
        cli,
        &format!("stackdiff {} → {}", from.describe(), to.describe()),
        &diffs,
        &options,
    );
    Ok(0)
}

fn main() {
    let cli = Cli::parse();

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
