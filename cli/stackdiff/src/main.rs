use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};

use stackdiff::calltree::build_call_tree;
use stackdiff::extract::{build_index, extract_functions, FunctionIndex};
use stackdiff::git::{
    assert_git_repo, list_source_files, read_snapshot_file, resolve_snapshots, verify_commit,
};
use stackdiff::infer::{diff_entry, infer_entries};
use stackdiff::render::{render_diff, RenderOptions};
use stackdiff::types::{DiffNode, DiffStatus, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
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
    #[arg(long, default_value_t = 12)]
    max_depth: usize,

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

fn load_index(
    cwd: &std::path::Path,
    snapshot: &Snapshot,
    path_filters: &[String],
) -> Result<FunctionIndex> {
    let files = list_source_files(cwd, snapshot, path_filters)?;
    let mut all = Vec::new();
    for file in files {
        let Some(source) = read_snapshot_file(cwd, snapshot, &file) else {
            continue;
        };
        match extract_functions(&file, &source) {
            Ok(functions) => all.extend(functions),
            Err(error) => eprintln!(
                "warn: failed to parse {file} @ {}: {error}",
                snapshot.describe()
            ),
        }
    }
    Ok(build_index(all))
}

fn tree_as_diff(node: &stackdiff::types::CallNode) -> DiffNode {
    DiffNode {
        key: node.key.clone(),
        label: node.label.clone(),
        kind: node.kind,
        status: DiffStatus::Same,
        children: node.children.iter().map(tree_as_diff).collect(),
    }
}

fn run_tree(cli: &Cli, cwd: &std::path::Path, color: bool) -> Result<i32> {
    let reference = cli.from.as_deref().or(cli.from_ref.as_deref());
    let snapshot = match reference {
        Some(reference) => {
            verify_commit(cwd, reference)?;
            Snapshot::Commit(reference.to_string())
        }
        None => Snapshot::Worktree,
    };
    let index = load_index(cwd, &snapshot, &cli.paths)?;

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

    println!("stackdiff --tree @ {}\n", snapshot.describe());
    let mut printed = 0;
    for entry in &cli.entries {
        let Some(resolved) = stackdiff::calltree::resolve_entry(entry, &index) else {
            eprintln!("Entrypoint not found: {entry}");
            continue;
        };
        if printed > 0 {
            println!();
        }
        let tree = build_call_tree(&resolved, &index, cli.max_depth);
        println!(
            "{}",
            render_diff(&tree_as_diff(&tree), RenderOptions { color })
        );
        printed += 1;
    }
    Ok(if printed > 0 { 0 } else { 1 })
}

fn run_diff(cli: &Cli, cwd: &std::path::Path, color: bool) -> Result<i32> {
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

    let before = load_index(cwd, &from, &cli.paths)?;
    let after = load_index(cwd, &to, &cli.paths)?;

    let entries = infer_entries(&before, &after, &cli.entries, cli.max_depth)?;

    if entries.is_empty() {
        println!(
            "No callstack changes between {} and {}.",
            from.describe(),
            to.describe()
        );
        return Ok(0);
    }

    println!("stackdiff {} → {}\n", from.describe(), to.describe());

    let mut printed = 0;
    for entry in &entries {
        let Some(diff) = diff_entry(entry, &before, &after, cli.max_depth) else {
            continue;
        };
        if printed > 0 {
            println!();
        }
        println!("{}", render_diff(&diff, RenderOptions { color }));
        printed += 1;
    }

    if printed == 0 {
        println!("No callstack changes for inferred entrypoints.");
    }

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
