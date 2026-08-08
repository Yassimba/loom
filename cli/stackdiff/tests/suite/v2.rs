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
    let plain = stackdiff::boxes::render_boxes(&diff, false);
    assert!(plain.contains("╭"), "draws rounded boxes:\n{plain}");
    assert!(
        plain.contains("│ two() │"),
        "boxes the added call:\n{plain}"
    );
    assert!(plain.contains("▼"), "draws connector arrows:\n{plain}");
    let colored = stackdiff::boxes::render_boxes(&diff, true);
    assert!(
        colored.contains("\u{1b}[32m"),
        "added box painted green:\n{colored:?}"
    );
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
