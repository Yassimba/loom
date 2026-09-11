#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn status(home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom"))
        .arg("status")
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LOOM_REPO_DIR", common::repo_root())
        .env("PATH", home.join("bin"))
        .env("NO_COLOR", "1")
        .current_dir(home)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn general_status_excludes_vault_scoped_packages() {
    let home = common::temp_home("status-wiki");
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let pi = bin.join("pi");
    let catalog = loom::Catalog::embedded().unwrap();

    // Pi lists project packages when status runs from inside a Vault, but not outside it.
    for project_packages in [
        "",
        "Project packages:\n  npm:@companion-ai/feynman@0.3.47\n",
    ] {
        fs::write(
            &pi,
            format!(
                "#!/bin/sh\nif [ \"$1\" = list ]; then\nprintf '%s\\n' 'User packages:\n  npm:pi-subagents@0.66.0\n{project_packages}'\nelse\nprintf '0.85.1\\n'\nfi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&pi, fs::Permissions::from_mode(0o755)).unwrap();
        let output = status(&home);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{text}");
        assert_eq!(
            text.lines().filter(|line| *line == "Wiki").count(),
            1,
            "{text}"
        );
        assert!(text.contains("no registered Vaults"), "{text}");
        let inventory = text
            .split_once("Selected resources")
            .unwrap()
            .1
            .split_once("Agent skills")
            .unwrap()
            .0;
        for resource in catalog.resources.iter().filter(|r| r.group == "Wiki") {
            assert!(
                !inventory.contains(&resource.label),
                "Vault-only {} leaked into general status:\n{inventory}",
                resource.label
            );
        }
        assert!(inventory
            .lines()
            .any(|line| { line.contains("subagents") && line.contains("catalog item installed") }));
        assert!(inventory.lines().any(|line| {
            line.contains("web-access") && line.contains("catalog item not installed")
        }));
    }
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn wiki_section_checks_each_registered_vault_not_global_packages() {
    use loom::wiki::{VaultRecord, WikiRegistry};

    let home = common::temp_home("status-vaults").canonicalize().unwrap();
    let ready = home.join("a-ready");
    let global_only = home.join("b-global-only");
    let missing = home.join("c-missing");
    for vault in [&ready, &global_only] {
        fs::create_dir_all(vault.join(".pi")).unwrap();
        fs::write(vault.join(".claude-obsidian.json"), "{}").unwrap();
    }
    // Global skills must not satisfy a Vault's local skill requirements.
    for root in [&home, &ready] {
        for name in ["qmd", "confluence-export"] {
            let skill = root.join(format!(".agents/skills/{name}/SKILL.md"));
            fs::create_dir_all(skill.parent().unwrap()).unwrap();
            fs::write(skill, "installed").unwrap();
        }
    }
    fs::write(
        ready.join(".pi/packages.txt"),
        format!(
            "Project packages:\n  {}/product\n  npm:@companion-ai/feynman@0.3.47\n",
            home.display()
        ),
    )
    .unwrap();
    let mut registry = WikiRegistry::default();
    registry.vaults = [&ready, &global_only, &missing]
        .into_iter()
        .map(|path| VaultRecord {
            path: path.clone(),
            feynman: true,
            confluence: true,
        })
        .collect();
    registry.save(&home).unwrap();
    let mut state = loom::InstallState::load(&home).unwrap();
    state.record(loom::OwnedResource {
        id: "skill:confluence-export".into(),
        scope: loom::OwnershipScope::Project {
            root: ready.clone(),
        },
        depends_on: Vec::new(),
        receipts: Vec::new(),
    });
    state.save(&home).unwrap();
    let selection = loom::manifest::conf_d_target(&home);
    fs::create_dir_all(selection.parent().unwrap()).unwrap();
    fs::write(&selection, format!(
        "[tools]\n\"{}\" = \"1\"\npython = \"1\"\n\"{}\" = \"1\"\n\"npm:@tobilu/qmd\" = \"1\"\n\"pipx:confluence-markdown-exporter\" = \"1\"\n",
        loom::wiki::PRODUCT_KEY, loom::manifest::PI_TOOL_KEY,
    )).unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for (name, script) in [
        ("pi", "#!/bin/sh\nif [ \"$1\" = list ]; then\nprintf '%s\\n' \"$PWD\" >> \"$HOME/pi-probes\"\nprintf 'User packages:\\n  %s/product\\n  npm:@companion-ai/feynman@0.3.47\\n' \"$HOME\"\nif [ -f .pi/packages.txt ]; then /bin/cat .pi/packages.txt; fi\nelse printf '0.85.1\\n'; fi\n"),
        ("mise", "#!/bin/sh\ncase \"$1\" in\nwhere) printf '%s/product\\n' \"$HOME\";;\nexec) printf '{\"schema\":\"claude-obsidian.doctor.v1\",\"ok\":true}\\n';;\n*) printf '1.0\\n';;\nesac\n"),
        ("python", "#!/bin/sh\nexit 0\n"),
        ("qmd", "#!/bin/sh\nexit 0\n"),
        ("cme", "#!/bin/sh\nexit 0\n"),
        ("obsidian", "#!/bin/sh\nexit 0\n"),
    ] {
        let path = bin.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = status(&home);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "{text}");
    assert_eq!(
        text.lines().filter(|line| *line == "Wiki").count(),
        1,
        "{text}"
    );
    let (general, wiki) = text.split_once("\nWiki\n").unwrap();
    for name in [
        "feynman",
        "claude-obsidian",
        "qmd",
        "confluence-markdown-exporter",
    ] {
        assert!(!general.contains(name), "{general}");
    }
    assert!(
        !general.contains("a-ready"),
        "Vault skill tree leaked into general status: {general}"
    );
    let (_, vaults) = wiki.split_once("a-ready").unwrap();
    let (installed, rest) = vaults.split_once("b-global-only").unwrap();
    let (absent, _) = rest.split_once("c-missing").unwrap();
    for name in ["claude-obsidian", "Feynman", "qmd", "Confluence"] {
        assert!(
            installed
                .lines()
                .any(|line| line.contains(name) && line.contains("ready")),
            "{installed}"
        );
        assert!(
            absent
                .lines()
                .any(|line| line.contains(name) && line.contains("missing")),
            "{absent}"
        );
    }
    assert!(wiki.contains("Obsidian"), "{wiki}");
    assert!(wiki.contains("missing; not recreated"), "{wiki}");
    assert!(!missing.exists());
    assert!(
        !text.contains("Selected resources, runtimes, and Wiki Vaults checked"),
        "{text}"
    );
    assert!(wiki.contains("Some checks need attention"), "{text}");
    assert!(text.contains("loom wiki"), "{text}");
    let probes = fs::read_to_string(home.join("pi-probes")).unwrap();
    assert_eq!(
        probes.lines().collect::<Vec<_>>(),
        [
            home.to_str().unwrap(),
            ready.to_str().unwrap(),
            global_only.to_str().unwrap()
        ]
    );

    // A healthy registered Vault contributes to the overall success verdict.
    registry.vaults.truncate(1);
    registry.save(&home).unwrap();
    let output = status(&home);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    // Unselected companions are optional; global copies still don't count as local installs.
    registry.vaults[0].feynman = false;
    registry.vaults[0].confluence = false;
    registry.save(&home).unwrap();
    fs::write(
        ready.join(".pi/packages.txt"),
        format!("Project packages:\n  {}/product\n", home.display()),
    )
    .unwrap();
    fs::remove_file(ready.join(".agents/skills/confluence-export/SKILL.md")).unwrap();
    let output = status(&home);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    for name in ["Feynman", "Confluence"] {
        assert!(
            text.lines()
                .any(|line| line.contains(name) && line.contains("not selected for this Vault")),
            "{text}"
        );
    }
    fs::remove_dir_all(home).unwrap();
}
