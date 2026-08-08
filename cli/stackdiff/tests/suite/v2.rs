//! v2 features: doc lines, return types, binding/args dataflow, locations,
//! pruning, and JSON output.

use crate::common::{diff_outdent, pretty_eq, rich_tree, sources_from_file_diff};

/// Expected trees here carry no +/- markers, so outdenting eats the status
/// column; restore it.
fn with_status_column(expected: &str) -> String {
    diff_outdent(expected)
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}
use stackdiff::calltree::build_call_tree;
use stackdiff::diff::{diff_trees, prune_unchanged};
use stackdiff::extract::{build_index, extract_functions};

#[test]
fn rich_render_shows_binding_args_returns_doc_and_location() {
    let actual = rich_tree(
        r#"
      /** Load the app config. */
      export function loadConfig(path: string): Config {
        return read(path);
      }
      export function boot() {
        const config = loadConfig("app.json");
        start(config);
      }
    "#,
        "boot",
        "app.ts",
    );
    println!("{actual}");
    pretty_eq(
        &actual,
        &with_status_column(
            r#"
      boot()  app.ts:5
      ├─ config = loadConfig("app.json") → Config  app.ts:2
      │  │  “Load the app config.”
      │  └─ read(path)
      └─ start(config)
    "#,
        ),
    );
}

#[test]
fn python_docstring_and_return_annotation_surface() {
    let actual = rich_tree(
        r#"
      def fetch(url) -> Response:
          """Fetch a URL. Retries twice."""
          return get(url)

      def main():
          resp = fetch("x")
          show(resp)
    "#,
        "main",
        "app.py",
    );
    println!("{actual}");
    pretty_eq(
        &actual,
        &with_status_column(
            r#"
      main()  app.py:5
      ├─ resp = fetch("x") → Response  app.py:1
      │  │  “Fetch a URL.”
      │  └─ get(url)
      └─ show(resp)
    "#,
        ),
    );
}

#[test]
fn rust_doc_comment_return_type_and_macro_calls() {
    let actual = rich_tree(
        r#"
      /// Add two numbers.
      pub fn add(a: i32, b: i32) -> i32 {
          a + b
      }
      pub fn main() {
          let total = add(1, 2);
          println!("{total}");
      }
    "#,
        "main",
        "app.rs",
    );
    println!("{actual}");
    pretty_eq(
        &actual,
        &with_status_column(
            r#"
      main()  app.rs:5
      ├─ total = add(1, 2) → i32  app.rs:2
      │     “Add two numbers.”
      └─ println!()
    "#,
        ),
    );
}

#[test]
fn prune_keeps_changes_with_context_and_elides_the_rest() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
        one();
        two();
        three();
        four();
    +   five();
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    let pruned = prune_unchanged(&diff, 1);
    let labels: Vec<&str> = pruned.children.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["…", "four()", "five()"]);
}

#[test]
fn call_site_changes_mark_amber_in_rich_and_hide_in_plain() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
    -   start(config);
    +   start(config, retries);
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    let rich = stackdiff::render::render_diff(
        &diff,
        &stackdiff::render::RenderOptions {
            rich: true,
            ..Default::default()
        },
    );
    assert!(
        rich.contains("! └─ start(config, retries)"),
        "rich render should mark the call-site change:\n{rich}"
    );
    let plain = stackdiff::render::render_diff(&diff, &stackdiff::render::RenderOptions::default());
    assert!(
        plain.contains("  └─ start()"),
        "plain render hides call-site-only changes:\n{plain}"
    );
}

#[test]
fn mermaid_output_marks_added_nodes() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
        one();
    +   two();
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    let mermaid = stackdiff::render::render_mermaid(&diff);
    assert!(mermaid.starts_with("flowchart TD"));
    assert!(mermaid.contains("n0 --> n2[\"two()\"]"));
    assert!(mermaid.contains("class n2 added"));
}

#[test]
fn test_files_are_recognized_by_convention() {
    use stackdiff::git::is_test_file;
    for test in [
        "packages/core/tests/application/test_runner.py",
        "src/__tests__/app.test.ts",
        "src/app.spec.tsx",
        "pkg/runner_test.go",
        "tests/conftest.py",
        "test_edge.py",
    ] {
        assert!(is_test_file(test), "{test} should count as a test file");
    }
    for source in [
        "src/app.ts",
        "packages/core/src/runner.py",
        "pkg/runner.go",
        "src/latest_results.py",
        "src/contest.py",
    ] {
        assert!(
            !is_test_file(source),
            "{source} should not count as a test file"
        );
    }
}

#[test]
fn boxes_render_draws_statused_boxes() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
        one();
    +   two();
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    use stackdiff::boxes::{render_boxes, BoxOptions, Direction};
    let plain = render_boxes(&diff, &BoxOptions::default());
    assert!(plain.contains("╭"), "draws rounded boxes:\n{plain}");
    assert!(
        plain.contains("│  two()  │"),
        "boxes the added call:\n{plain}"
    );
    assert!(plain.contains("▶"), "left-right arrows:\n{plain}");
    let colored = render_boxes(
        &diff,
        &BoxOptions {
            color: true,
            ..Default::default()
        },
    );
    assert!(
        colored.contains("\u{1b}[38;2;63;185;80m"),
        "added box painted green:\n{colored:?}"
    );
    assert!(
        colored.contains("\u{1b}[48;2;14;40;22m"),
        "added box carries its background tint:\n{colored:?}"
    );
    let td = render_boxes(
        &diff,
        &BoxOptions {
            dir: Direction::TopDown,
            ..Default::default()
        },
    );
    assert!(td.contains("▼"), "top-down keeps vertical arrows:\n{td}");
}

#[test]
fn sequence_view_orders_messages_and_marks_changes() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
        one();
    +   two();
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    let (source, marks) = stackdiff::views::sequence_mermaid(&diff);
    assert!(source.starts_with("sequenceDiagram"), "{source}");
    assert!(source.contains("two()"), "{source}");
    assert!(
        marks
            .iter()
            .any(|mark| mark.label == "two()"
                && mark.status == stackdiff::types::DiffStatus::Added),
        "{marks:?}"
    );
    let colored = stackdiff::views::render_colored(&source, &marks, true, Some(100)).unwrap();
    assert!(
        colored.contains("\u{1b}[38;2;63;185;80mtwo()\u{1b}[0m"),
        "{colored:?}"
    );
}

#[test]
fn class_view_groups_methods_by_type() {
    let source = diff_outdent(
        r#"
      class Runner {
        start() { this.prepare(); }
        prepare() {}
      }
    "#,
    );
    let index = build_index(extract_functions("app.ts", &source).unwrap());
    let tree = build_call_tree("Runner.start", &index, 12);
    let diff = diff_trees(&tree, &tree);
    let (mermaid, _) =
        stackdiff::views::class_mermaid(&[diff], &std::collections::BTreeMap::new()).unwrap();
    assert!(mermaid.contains("class Runner {"), "{mermaid}");
    assert!(mermaid.contains("+start()"), "{mermaid}");
    assert!(mermaid.contains("+prepare()"), "{mermaid}");
}

#[test]
fn noise_filter_hides_builtins_and_collapses_repeats() {
    use stackdiff::noise::{scrub_index, NoiseFilter};
    let source = diff_outdent(
        r#"
      export function boot() {
        real_work();
        real_work();
        len(items);
        custom_helper(items);
      }
    "#,
    );
    let mut index = build_index(extract_functions("app.ts", &source).unwrap());
    let filter = NoiseFilter::default_enabled();
    scrub_index(&mut index, &filter);
    let tree = build_call_tree("boot", &index, 12);
    let labels: Vec<&str> = tree.children.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["real_work() ×2", "custom_helper()"],
        "{labels:?}"
    );
}

#[test]
fn caller_tree_inverts_the_graph() {
    use stackdiff::calltree::{build_caller_tree, reverse_index};
    let source = diff_outdent(
        r#"
      export function helper() {}
      export function alpha() { helper(); }
      export function beta() { helper(); }
    "#,
    );
    let index = build_index(extract_functions("app.ts", &source).unwrap());
    let reverse = reverse_index(&index);
    let tree = build_caller_tree("helper", &index, &reverse, 3);
    let mut callers: Vec<&str> = tree.children.iter().map(|c| c.key.as_str()).collect();
    callers.sort();
    assert_eq!(callers, vec!["alpha", "beta"]);
}

#[test]
fn er_view_includes_extracted_fields() {
    let source = diff_outdent(
        r#"
      class Runner {
        retries: number;
        start() { this.prepare(); }
        prepare() {}
      }
    "#,
    );
    let types_list = stackdiff::extract::extract_types("app.ts", &source).unwrap();
    let mut types = std::collections::BTreeMap::new();
    for info in types_list {
        types.insert(info.name, info.fields);
    }
    let index = build_index(extract_functions("app.ts", &source).unwrap());
    let tree = build_call_tree("Runner.start", &index, 12);
    let diff = diff_trees(&tree, &tree);
    let (mermaid, _) = stackdiff::views::class_mermaid(&[diff], &types).unwrap();
    assert!(mermaid.contains("+retries number"), "{mermaid}");
}

#[test]
fn twin_calls_align_by_argument_similarity() {
    let diff_src = diff_outdent(
        r#"
      export function boot() {
    +   wrap(alpha);
        wrap(beta);
      }
    "#,
    );
    let (before_src, after_src) = sources_from_file_diff(&diff_src);
    let before = build_index(extract_functions("file.before.ts", &before_src).unwrap());
    let after = build_index(extract_functions("file.after.ts", &after_src).unwrap());
    let diff = diff_trees(
        &build_call_tree("boot", &before, 12),
        &build_call_tree("boot", &after, 12),
    );
    // The pre-existing wrap(beta) must read unchanged; wrap(alpha) is the add.
    let statuses: Vec<(String, stackdiff::types::DiffStatus)> = diff
        .children
        .iter()
        .map(|c| (c.meta.args.join(","), c.status))
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("alpha".to_string(), stackdiff::types::DiffStatus::Added),
            ("beta".to_string(), stackdiff::types::DiffStatus::Same),
        ],
        "{statuses:?}"
    );
}

#[test]
fn lineage_view_draws_shared_callees_once() {
    let source = diff_outdent(
        r#"
      export function resolve(): Plan { return read(); }
      export function alpha() { const plan = resolve(); }
      export function beta() { const p = resolve(); }
      export function boot() { alpha(); beta(); }
    "#,
    );
    let index = build_index(extract_functions("app.ts", &source).unwrap());
    let tree = build_call_tree("boot", &index, 12);
    let diff = diff_trees(&tree, &tree);
    let (nodes, edges) = match stackdiff::views::lineage_graph(&[diff], None, None).unwrap() {
        stackdiff::views::Lineage::Graph(nodes, edges) => (nodes, edges),
        _ => panic!("expected graph"),
    };
    assert_eq!(
        nodes.iter().filter(|n| n.key == "resolve").count(),
        1,
        "shared callee appears once"
    );
    let fan_in = edges.iter().filter(|e| e.to == "resolve").count();
    assert_eq!(fan_in, 2, "two edges converge on resolve");
    assert!(
        edges
            .iter()
            .any(|e| e.label.as_deref() == Some("plan") || e.label.as_deref() == Some("p")),
        "binding rides the edge"
    );
    let drawn = stackdiff::dag::render_dag(&nodes, &edges, false);
    assert_eq!(
        drawn.matches("resolve → Plan").count(),
        1,
        "one box for the shared callee:\n{drawn}"
    );
    assert!(drawn.contains("▶"), "arrows drawn:\n{drawn}");
}

#[test]
fn json_output_carries_dataflow_fields() {
    let source = diff_outdent(
        r#"
      export function boot() {
        const config = loadConfig();
        start(config);
      }
    "#,
    );
    let index = build_index(extract_functions("app.ts", &source).unwrap());
    let tree = build_call_tree("boot", &index, 12);
    let diff = diff_trees(&tree, &tree);
    let json = serde_json::to_value(&diff).unwrap();
    let children = json["children"].as_array().unwrap();
    assert_eq!(children[0]["binding"], "config");
    assert_eq!(children[1]["args"][0], "config");
    assert_eq!(children[1]["consumes"][0], "config");
    assert_eq!(json["location"], "app.ts:1");
}
