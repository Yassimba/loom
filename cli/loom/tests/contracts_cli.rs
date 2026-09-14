mod common;

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn contracts(home: &Path, project: &Path, args: &[&str]) -> Output {
    let before = [
        loom::digest_path(home).unwrap(),
        loom::digest_path(project).unwrap(),
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_loom"))
        .arg("contracts")
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LOOM_BOOTSTRAP", "1")
        .env("PATH", home.join("bin"))
        .env("NO_COLOR", "1")
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        before,
        [
            loom::digest_path(home).unwrap(),
            loom::digest_path(project).unwrap()
        ],
        "checker changed files: {args:?}"
    );
    output
}

fn report(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).expect("exactly one JSON object");
    assert!(value.is_object());
    value
}

#[test]
fn contracts_check_reports_structure_without_writing_files() {
    // Public seam: binary inputs, diagnostics, exit status, and read-only effects.
    // Catches validation accidentally passing through setup/bootstrap mutation.
    let home = common::temp_home("contracts-home");
    let project = common::temp_home("contracts-project");
    fs::create_dir(project.join(".git")).unwrap();
    let source = "@cc [owner:alice;bob,owner:carol,custom:;] stable-id\nKeep this obligation.\n";
    fs::write(project.join("CONTRACTS"), source).unwrap();

    for args in [
        vec!["--json", "check"],
        vec!["check", "CONTRACTS", "--json"],
    ] {
        assert_eq!(
            report(&contracts(&home, &project, &args), 0),
            json!({"checked": 1, "diagnostics": []})
        );
    }
    let text = contracts(&home, &project, &["check"]);
    assert!(text.status.success(), "{text:?}");
    assert!(String::from_utf8_lossy(&text.stdout).contains("1 file read"));

    // File identity is scoped: siblings may reuse IDs, descendants may not.
    for directory in ["z", "a", ".git/hidden"] {
        fs::create_dir_all(project.join(directory)).unwrap();
    }
    fs::write(project.join(".git/hidden/CONTRACTS"), "@cc bad").unwrap();
    // Supported source files are checked too; unsupported ones are skipped.
    fs::write(
        project.join("scanned.rs"),
        "/// @cc attached\n/// Body\nfn scanned() {}\n",
    )
    .unwrap();
    fs::write(project.join("notes.txt"), "@cc malformed").unwrap();
    fs::write(project.join("z/CONTRACTS"), "@cc shared\nBody\n").unwrap();
    fs::write(project.join("a/CONTRACTS"), "@cc shared\nBody\n").unwrap();
    assert_eq!(
        report(&contracts(&home, &project, &["check", "--json"]), 0),
        json!({"checked": 4, "diagnostics": []})
    );
    fs::write(
        project.join("a/CONTRACTS"),
        "@cc stable-id\nBody\n@cc stable-id\nAgain\n",
    )
    .unwrap();
    for target in ["a", "a/CONTRACTS"] {
        let value = report(&contracts(&home, &project, &["check", target, "--json"]), 1);
        assert_eq!(value["checked"], 2);
        let diagnostics = value["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 2, "{value}");
        assert_eq!(
            diagnostics[0]["path"],
            json!(Path::new("a").join("CONTRACTS"))
        );
        assert_eq!(diagnostics[0]["line"], 1);
        assert!(diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("CONTRACTS:1"));
        assert_eq!(diagnostics[1]["line"], 3);
    }
    let text = contracts(&home, &project, &["check", "a"]);
    assert_eq!(text.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&text.stdout)
        .contains(&format!("{}:1", Path::new("a").join("CONTRACTS").display())));

    let malformed = "unexpected preamble\n@cc [owner:] invalid\nBody\n@cc\n@cc no-body\n\n@cc [custom:a,custom:b] good\nBody\n@cc two ids\nBody\n@cc [owner:a]missing-space\nBody\n@cc\ttab\nBody\n@cc bad:id\nBody\n@cc [owner:a,,label:b] commas\nBody\n";
    fs::write(project.join("z/CONTRACTS"), malformed).unwrap();
    let output = contracts(&home, &project, &["--json", "check"]);
    let value = report(&output, 1);
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 11, "{value}");
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|d| d["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [1, 3, 1, 2, 4, 5, 9, 11, 13, 15, 17]);
    assert_eq!(
        output.stdout,
        contracts(&home, &project, &["check", "--json"]).stdout
    );
    assert_eq!(
        fs::read_to_string(project.join("z/CONTRACTS")).unwrap(),
        malformed
    );

    for invalid in [
        "@cc [owner:alice incomplete",
        "@cc [] empty-metadata",
        "@cc [owner] missing-colon",
        "@cc [owner:a b] whitespace",
        "@cc bad[id]",
        "@cc [owner:a]",
    ] {
        fs::write(
            project.join("z/CONTRACTS"),
            format!("{invalid}\nBody\n@cc valid\nBody\n"),
        )
        .unwrap();
        let value = report(
            &contracts(&home, &project, &["check", "z/CONTRACTS", "--json"]),
            1,
        );
        assert_eq!(
            value["diagnostics"].as_array().unwrap().len(),
            1,
            "{invalid}: {value}"
        );
        assert_eq!(value["diagnostics"][0]["line"], 1);
    }

    for args in [
        vec!["--json", "check", "missing"],
        vec!["check", "notes.txt", "--json"],
        vec!["--json", "check", "--unknown"],
        vec!["check", "--unknown", "--json"],
        vec!["--json"],
        vec!["--json", "no-such-command"],
    ] {
        let value = report(&contracts(&home, &project, &args), 2);
        assert!(!value["errors"].as_array().unwrap().is_empty(), "{value}");
        assert!(value.get("diagnostics").is_none(), "{value}");
    }
    assert_eq!(
        contracts(&home, &project, &["check", "missing"])
            .status
            .code(),
        Some(2)
    );
    for args in [
        vec!["--help"],
        vec!["check", "--help"],
        vec!["--json", "check", "--help"],
    ] {
        let help = contracts(&home, &project, &args);
        assert!(help.status.success(), "{help:?}");
        assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
    }
    assert_eq!(
        fs::read_to_string(project.join("CONTRACTS")).unwrap(),
        source
    );
    assert_eq!(
        fs::read_dir(&home).unwrap().count(),
        0,
        "bootstrap wrote to HOME"
    );
    filesystem_scope(&home);
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}

fn filesystem_scope(home: &Path) {
    let project = common::temp_home("contracts-scope");
    fs::create_dir_all(project.join("child/grandchild")).unwrap();
    fs::write(project.join("CONTRACTS"), "\n\t\n").unwrap();
    assert_eq!(
        report(&contracts(home, &project, &["check", "--json"]), 0),
        json!({"checked": 1, "diagnostics": []})
    );
    fs::write(project.join("CONTRACTS"), "@cc missing-prose").unwrap();
    let value = report(&contracts(home, &project, &["check", "--json"]), 1);
    assert_eq!(
        value["diagnostics"][0]["message"],
        "contract prose must be non-empty"
    );
    fs::write(project.join("CONTRACTS"), "@cc repeated\nRoot\n").unwrap();
    fs::write(project.join("child/CONTRACTS"), "@cc repeated\nChild\n").unwrap();
    // Without a repository marker, the selected directory is the scope root.
    assert_eq!(
        report(&contracts(home, &project, &["check", "child", "--json"]), 0),
        json!({"checked": 1, "diagnostics": []})
    );
    // A worktree's .git file supplies the same root boundary as a .git directory.
    fs::write(project.join(".git"), "gitdir: elsewhere\n").unwrap();
    let output = contracts(home, &project, &["check", "child/grandchild", "--json"]);
    let value = report(&output, 1);
    assert_eq!(value["checked"], 2);
    assert_eq!(
        value["diagnostics"][0]["path"],
        json!(Path::new("child").join("CONTRACTS"))
    );

    // An undecodable file is a finding, not an error that hides the whole tree.
    fs::write(project.join("child/CONTRACTS"), [0xff]).unwrap();
    let value = report(&contracts(home, &project, &["check", "--json"]), 1);
    assert_eq!(
        value["diagnostics"][0]["message"], "file is not valid UTF-8",
        "{value}"
    );
    fs::write(
        project.join("child/CONTRACTS"),
        "\r\n@cc [x:a;;b] unicode-λ\r\nMarkdown **body**\r\n",
    )
    .unwrap();
    assert_eq!(
        report(&contracts(home, &project, &["check", "--json"]), 0)["checked"],
        2
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = common::temp_home("contracts-outside");
        fs::write(outside.join("CONTRACTS"), "@cc invalid").unwrap();
        symlink(&outside, project.join("linked-directory")).unwrap();
        symlink(&project, project.join("loop")).unwrap();
        fs::create_dir(project.join("linked-file")).unwrap();
        symlink(
            outside.join("CONTRACTS"),
            project.join("linked-file/CONTRACTS"),
        )
        .unwrap();
        assert_eq!(
            report(&contracts(home, &project, &["check", "--json"]), 0)["checked"],
            2
        );
        for target in ["linked-directory", "linked-file/CONTRACTS"] {
            report(&contracts(home, &project, &["check", target, "--json"]), 2);
        }
        // Linux permits non-UTF-8 names; macOS filesystems reject their creation.
        #[cfg(target_os = "linux")]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = project.join(OsString::from_vec(vec![0xff]));
            fs::create_dir(&non_utf8).unwrap();
            for content in ["@cc invalid", "@cc valid\nBody\n"] {
                fs::write(non_utf8.join("CONTRACTS"), content).unwrap();
                for command in ["check", "list"] {
                    let value = report(&contracts(home, &project, &[command, ".", "--json"]), 2);
                    assert!(
                        value["errors"][0]["message"]
                            .as_str()
                            .unwrap()
                            .contains("UTF-8"),
                        "{value}"
                    );
                }
            }
        }
        fs::remove_dir_all(outside).unwrap();
    }
    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn contracts_closed_stdout_is_an_io_error_not_a_panic() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    let root = common::temp_home("contracts-closed-stdout");
    fs::create_dir(root.join("home")).unwrap();
    fs::create_dir(root.join("clean")).unwrap();
    fs::create_dir(root.join("findings")).unwrap();
    fs::write(root.join("clean/CONTRACTS"), "@cc valid\nBody\n").unwrap();
    fs::write(root.join("findings/CONTRACTS"), "@cc empty\n").unwrap();
    let mut failures = Vec::new();
    for args in [
        vec!["check", "clean", "--json"],
        vec!["check", "findings", "--json"],
        vec!["check", "missing", "--json"],
        vec!["check", "--unknown", "--json"],
        vec!["check", "clean"],
        vec!["check", "findings"],
        vec!["check", "missing"],
    ] {
        let before = loom::digest_path(&root).unwrap();
        let (writer, reader) = UnixStream::pair().unwrap();
        reader.shutdown(std::net::Shutdown::Both).unwrap();
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_loom"))
            .arg("contracts")
            .args(&args)
            .env("HOME", root.join("home"))
            .env("USERPROFILE", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("home/.config"))
            .env("LOOM_BOOTSTRAP", "1")
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() != Some(2) || stderr.contains("panicked") {
            failures.push(format!("{args:?}: {:?}, {stderr}", output.status));
        }
        assert_eq!(before, loom::digest_path(&root).unwrap());
    }
    fs::remove_dir_all(root).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A repository fixture with attached contracts in every supported language.
fn attachment_project() -> std::path::PathBuf {
    let project = common::temp_home("contracts-attach");
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("CONTRACTS"),
        "@cc directory-rule\nRoot rule.\n",
    )
    .unwrap();
    fs::write(
        project.join("src/pay.rs"),
        "use std::fmt;\n\n/// @cc [owner:alice] balance-pre\n/// Balance covers the amount.\n#[inline]\npub fn pay(amount: u64) -> bool {\n    let ok = amount > 0;\n    ok\n}\n\n/// @cc balance-pre\n/// A different declaration may reuse the identifier.\npub fn refund() {}\n",
    )
    .unwrap();
    fs::write(
        project.join("src/pay.py"),
        "import os\n\n@decorator\ndef pay(amount):\n    \"\"\"\n    @cc python-pre\n    Amount is positive.\n    \"\"\"\n    return amount\n\ndef refund():\n    return 0\n",
    )
    .unwrap();
    fs::write(
        project.join("src/pay.ts"),
        "export const rate = 1;\n\n/**\n * @cc ts-pre\n * Amount is positive.\n */\nexport function pay(amount: number) {\n  return amount;\n}\n",
    )
    .unwrap();
    project
}

#[test]
fn contracts_attach_to_declarations_and_answer_at_and_list() {
    // Public seam: the binary's discovery commands over a real repository tree.
    let home = common::temp_home("contracts-attach-home");
    let project = attachment_project();

    // Every supported language attaches its contract to the following declaration.
    let value = report(&contracts(&home, &project, &["list", "src", "--json"]), 0);
    let listed = value["contracts"].as_array().unwrap();
    let ids: Vec<_> = listed
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        ["python-pre", "balance-pre", "balance-pre", "ts-pre"],
        "{value}"
    );
    for contract in listed {
        let declaration = &contract["scope"]["declaration"];
        assert!(!declaration["name"].as_str().unwrap().is_empty(), "{value}");
    }

    // `at` merges ancestor CONTRACTS with the declarations enclosing the line.
    let value = report(
        &contracts(&home, &project, &["at", "src/pay.rs:7", "--json"]),
        0,
    );
    let at: Vec<_> = value["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(at, ["directory-rule", "balance-pre"], "{value}");
    assert_eq!(value["contracts"][0]["scope"], json!("directory"));
    // A trailing column is accepted and ignored.
    assert_eq!(
        contracts(&home, &project, &["--json", "at", "src/pay.rs:7:3"]).stdout,
        contracts(&home, &project, &["at", "src/pay.rs:7", "--json"]).stdout
    );

    // A line outside any declaration still reports the directory obligations.
    let value = report(
        &contracts(&home, &project, &["at", "src/pay.rs:1", "--json"]),
        0,
    );
    assert_eq!(value["contracts"].as_array().unwrap().len(), 1, "{value}");
    // A location with no applicable contract at all is an empty success.
    fs::create_dir_all(project.join("other")).unwrap();
    fs::write(project.join("other/plain.rs"), "fn main() {}\n").unwrap();
    fs::remove_file(project.join("CONTRACTS")).unwrap();
    let value = report(
        &contracts(&home, &project, &["at", "other/plain.rs:1", "--json"]),
        0,
    );
    assert_eq!(value, json!({"contracts": []}));
    fs::write(
        project.join("CONTRACTS"),
        "@cc directory-rule\nRoot rule.\n",
    )
    .unwrap();

    // `list` resolves a single file, and an identifier across the repository.
    let value = report(
        &contracts(&home, &project, &["list", "src/pay.ts", "--json"]),
        0,
    );
    assert_eq!(value["contracts"].as_array().unwrap().len(), 1, "{value}");
    let value = report(
        &contracts(&home, &project, &["list", "ts-pre", "--json"]),
        0,
    );
    assert_eq!(value["contracts"][0]["id"], "ts-pre", "{value}");
    assert!(value["contracts"][0]["prose"]
        .as_str()
        .unwrap()
        .contains("Amount is positive"));
    // An identifier that matches nothing is a finding, not an error.
    report(
        &contracts(&home, &project, &["list", "absent-id", "--json"]),
        1,
    );
    let text = contracts(&home, &project, &["list", "ts-pre"]);
    assert!(text.status.success(), "{text:?}");
    assert!(
        String::from_utf8_lossy(&text.stdout).contains("ts-pre"),
        "{text:?}"
    );

    // Identifiers repeat freely across declarations; within one they are findings.
    let value = report(&contracts(&home, &project, &["check", "--json"]), 0);
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 0, "{value}");
    fs::write(
        project.join("src/pay.ts"),
        "/**\n * @cc ts-pre\n * Amount is positive.\n */\n/**\n * @cc ts-pre\n * Repeated on the same declaration.\n */\nexport function pay(amount: number) {\n  return amount;\n}\n",
    )
    .unwrap();
    let value = report(&contracts(&home, &project, &["check", "--json"]), 1);
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{value}");
    assert!(diagnostics[0]["message"]
        .as_str()
        .unwrap()
        .contains("duplicate contract id `ts-pre`"));

    // Malformed directives in source files are visible to check.
    fs::write(
        project.join("src/pay.ts"),
        "export const rate = 1;\n\n/**\n * @cc\n */\nexport function pay() {}\n",
    )
    .unwrap();
    let value = report(
        &contracts(&home, &project, &["check", "src/pay.ts", "--json"]),
        1,
    );
    assert_eq!(value["diagnostics"][0]["line"], 4, "{value}");

    // Usage failures stay operational errors.
    for args in [
        vec!["at", "src/pay.rs", "--json"],
        vec!["at", "src/pay.rs:zero", "--json"],
        vec!["at", "missing.rs:1", "--json"],
        vec!["list", "--json"],
        // A mistyped path is an error, not an identifier that matches nothing.
        vec!["list", "src/paay.rs", "--json"],
        vec!["list", "src/missing/", "--json"],
    ] {
        let value = report(&contracts(&home, &project, &args), 2);
        assert!(!value["errors"].as_array().unwrap().is_empty(), "{value}");
    }

    // One non-UTF-8 source is a finding; it must not hide the rest of the tree.
    fs::write(project.join("src/binary.rs"), [b'/', b'/', b'/', 0xff]).unwrap();
    let value = report(&contracts(&home, &project, &["check", "--json"]), 1);
    assert!(value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["message"] == "file is not valid UTF-8"));
    let value = report(
        &contracts(&home, &project, &["list", "python-pre", "--json"]),
        0,
    );
    assert_eq!(value["contracts"][0]["id"], "python-pre", "{value}");
    fs::remove_file(project.join("src/binary.rs")).unwrap();

    // Repeated runs are byte-identical and never write.
    assert_eq!(
        contracts(&home, &project, &["list", "src", "--json"]).stdout,
        contracts(&home, &project, &["list", "src", "--json"]).stdout
    );
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}

/// `--no-global` answers "what does this declaration itself promise", leaving
/// out the directory rules every file under a `CONTRACTS` inherits. A second
/// directive in one comment is a format error, reported as such.
#[test]
fn no_global_drops_directory_contracts_and_a_shared_comment_is_a_finding() {
    let home = common::temp_home("e2e-no-global-home");
    let project = common::temp_home("e2e-no-global-project");
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        project.join("CONTRACTS"),
        "@cc [owner:root] dir-rule\nDirectory obligation.\n",
    )
    .unwrap();
    fs::write(
        project.join("pay.rs"),
        "/// @cc first-in-comment\n/// @cc second-in-comment\n/// Only the second keeps the prose.\npub fn pay() {}\n",
    )
    .unwrap();

    let all = report(
        &contracts(&home, &project, &["at", "pay.rs:4", "--json"]),
        0,
    );
    assert_eq!(all["contracts"][0]["id"], "dir-rule", "{all}");
    let local = report(
        &contracts(
            &home,
            &project,
            &["at", "pay.rs:4", "--no-global", "--json"],
        ),
        0,
    );
    assert!(
        local["contracts"]
            .as_array()
            .is_some_and(|found| found.iter().all(|c| c["scope"] != "directory")),
        "{local}"
    );

    let checked = report(&contracts(&home, &project, &["check", "--json"]), 1);
    assert_eq!(
        checked["diagnostics"][0]["message"], "a documentation comment carries one @cc directive",
        "{checked}"
    );
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}

/// `related` needs a language server; without one it must name the missing
/// binary rather than report an empty result. `PATH` here holds no servers.
#[test]
fn related_names_the_tool_it_cannot_find() {
    let home = common::temp_home("e2e-related-home");
    let project = common::temp_home("e2e-related-project");
    fs::write(project.join("pay.rs"), "pub fn charge() {}\n").unwrap();
    fs::write(project.join("notes.md"), "not code\n").unwrap();

    let missing = report(
        &contracts(&home, &project, &["related", "pay.rs:1", "--json"]),
        2,
    );
    assert!(
        missing["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("rust-analyzer")),
        "{missing}"
    );

    let unsupported = report(
        &contracts(&home, &project, &["related", "notes.md:1", "--json"]),
        2,
    );
    assert!(
        unsupported["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains(".md")),
        "{unsupported}"
    );
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}

/// `diff` pairs contracts by identity across revisions, `affected --stale`
/// keeps only attached contracts whose code moved without their text, and
/// `check` demands that a `test:` anchor names a real identifier.
#[cfg(unix)]
#[test]
fn diff_stale_and_test_anchors_follow_one_change() {
    let home = common::temp_home("e2e-diff-home");
    let project = common::temp_home("e2e-diff-project");
    // The harness empties PATH; `diff` and `affected` need git on it.
    fs::create_dir(home.join("bin")).unwrap();
    let real_git =
        String::from_utf8(Command::new("which").arg("git").output().unwrap().stdout).unwrap();
    std::os::unix::fs::symlink(real_git.trim(), home.join("bin/git")).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            // Fixture commits must finish all writes before read-only snapshots.
            .args(["-c", "maintenance.auto=false"])
            .args(args)
            .current_dir(&project)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    };
    git(&["init", "-q"]);
    fs::write(project.join("CONTRACTS"), "@cc dir-rule\nDirectory.\n").unwrap();
    fs::write(
        project.join("pay.rs"),
        "/// @cc [owner:alice,test:pays_once] kept\n/// Charges once.\nfn pay() {\n    1\n}\n\n/// @cc reworded\n/// Old text.\nfn refund() {}\n\n/// @cc gone\n/// Removed later.\nfn cancel() {}\n\n#[test]\nfn pays_once() {}\n",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    assert_eq!(
        report(&contracts(&home, &project, &["check", "--json"]), 0)["diagnostics"],
        json!([])
    );

    // Change the body under `kept`, reword `reworded`, drop `gone`, add `fresh`.
    fs::write(
        project.join("pay.rs"),
        "/// @cc [owner:alice,test:pays_once] kept\n/// Charges once.\nfn pay() {\n    2\n}\n\n/// @cc reworded\n/// New text.\nfn refund() {}\n\nfn cancel() {}\n\n/// @cc [test:missing_test] fresh\n/// Added.\nfn hold() {}\n\n#[test]\nfn pays_once() {}\n",
    )
    .unwrap();

    let value = report(
        &contracts(&home, &project, &["diff", "--base", "HEAD", "--json"]),
        1,
    );
    assert_eq!(value["added"][0]["id"], "fresh", "{value}");
    assert_eq!(
        value["changed"][0]["before"]["prose"], "Old text.",
        "{value}"
    );
    assert_eq!(
        value["changed"][0]["after"]["prose"], "New text.",
        "{value}"
    );
    assert_eq!(value["removed"][0]["id"], "gone", "{value}");
    assert_eq!(value["removed"][0]["metadata"], json!([]), "{value}");

    let ids = |args: &[&str], code: i32| -> Vec<String> {
        report(&contracts(&home, &project, args), code)["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_owned())
            .collect()
    };
    // A prose-only edit is not a code change; `diff` reports it instead.
    assert_eq!(
        ids(&["affected", "--json"], 0),
        ["dir-rule", "kept", "fresh"]
    );
    // Only `kept` had its code touched while its text stayed put.
    assert_eq!(ids(&["affected", "--stale", "--json"], 0), ["kept"]);

    // `cancel` lost its contract and `hold` is new: both bodies are one line,
    // so only a declaration with a body is proposed. Give `cancel` one.
    fs::write(
        project.join("pay.rs"),
        "/// @cc [owner:alice,test:pays_once] kept\n/// Charges once.\nfn pay() {\n    2\n}\n\n/// @cc reworded\n/// New text.\nfn refund() {}\n\nfn cancel() {\n    0\n}\n\n/// @cc [test:missing_test] fresh\n/// Added.\nfn hold() {}\n\n#[test]\nfn pays_once() {}\n",
    )
    .unwrap();
    let value = report(&contracts(&home, &project, &["propose", "--json"]), 1);
    let proposals = value["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 1, "{value}");
    assert_eq!(proposals[0]["name"], "fn cancel() {", "{value}");
    assert_eq!(proposals[0]["governed_by"][0]["id"], "dir-rule", "{value}");

    let value = report(&contracts(&home, &project, &["check", "--json"]), 1);
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 1, "{value}");
    assert!(value["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("missing_test"));

    // Committing everything leaves nothing to report.
    git(&["add", "."]);
    git(&["commit", "-qm", "change"]);
    // A stale stat cache must not let read-only Git commands rewrite .git/index.
    for (index, command) in ["diff", "affected", "propose"].into_iter().enumerate() {
        fs::File::open(project.join("pay.rs"))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(index as u64 + 1),
            ))
            .unwrap();
        report(&contracts(&home, &project, &[command, "--json"]), 0);
    }
    let value = report(&contracts(&home, &project, &["diff", "--json"]), 0);
    assert_eq!(value, json!({"added": [], "changed": [], "removed": []}));
    let value = report(&contracts(&home, &project, &["propose", "--json"]), 0);
    assert_eq!(value, json!({"proposals": []}));
    let text = contracts(&home, &project, &["diff"]);
    assert!(text.status.success(), "{text:?}");
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn contract_prose_may_start_with_a_longer_cc_word() {
    let root = common::temp_home("e2e-contract-prose");
    fs::write(
        root.join("CONTRACTS"),
        "@cc stable\nAccount references follow.\n@ccount is a handle.\n@cc-check is a tool.\n@cc\tbroken\nBody.\n",
    )
    .unwrap();
    let report = loom::contracts::check(&root).unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].line, 5);
    fs::remove_dir_all(root).unwrap();
}
