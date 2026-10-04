mod common;
use common::install;
use loom::testing::{ok, ScriptedSystem};
use loom::{InstallPlan, InstallStep, Operation};
use std::fs;

fn package(target: &str) -> InstallStep {
    InstallStep {
        target: target.into(),
        operation: Operation::PiPackage {
            spec: target.into(),
            name: target.trim_start_matches("pi-package:").into(),
            project: false,
        },
    }
}

#[test]
fn package_install_applies_pi_compatibility_fixes() {
    let home = std::env::temp_dir().join(format!(
        "loom-pi-compat-install-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let agent = home.join(".pi/agent");
    let feynman = agent.join("npm/node_modules/@companion-ai/feynman/extensions/research-tools.ts");
    fs::create_dir_all(feynman.parent().unwrap()).unwrap();
    fs::write(
        &feynman,
        "import { registerThinkingCommand } from \"./research-tools/thinking.js\";\n\
         export default function researchTools(pi: unknown) {\n\
         \tregisterThinkingCommand(pi);\n\
         }\n",
    )
    .unwrap();
    let plan = InstallPlan {
        steps: vec![
            package("pi-package:pi-autoresearch"),
            package("pi-package:@companion-ai/feynman"),
        ],
    };

    let system = ScriptedSystem::new()
        .home(&home)
        .cwd(home.join("project"))
        .otherwise(ok(
            "User packages:\n  npm:pi-autoresearch\n  npm:@companion-ai/feynman\n",
        ));

    let report = install(&plan, &system);

    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(
        fs::read_to_string(agent.join("extensions/pi-autoresearch.json"))
            .unwrap()
            .contains("ctrl+shift+y")
    );
    assert!(!fs::read_to_string(feynman)
        .unwrap()
        .contains("registerThinkingCommand"));
    fs::remove_dir_all(home).unwrap();
}
