use loom::{
    execute_install_plan, CommandResult, CommandSpec, InstallPlan, InstallStep, StepAction, System,
};
use std::fs;
use std::path::PathBuf;

struct FakeSystem {
    home: PathBuf,
}

impl System for FakeSystem {
    fn command_exists(&self, _name: &str) -> bool {
        true
    }

    fn refresh_path(&self) {}

    fn run(&self, _command: &CommandSpec) -> anyhow::Result<CommandResult> {
        Ok(CommandResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn home_dir(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }

    fn current_dir(&self) -> Option<PathBuf> {
        Some(self.home.join("project"))
    }
}

fn package(target: &str) -> InstallStep {
    InstallStep {
        target: target.into(),
        manager: "pi".into(),
        action: StepAction::Command(CommandSpec::new("pi", ["install", target])),
        verification: None,
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
        prerequisites: Vec::new(),
        resources: vec![
            package("pi-package:pi-autoresearch"),
            package("pi-package:@companion-ai/feynman"),
        ],
    };

    let report = execute_install_plan(&plan, &FakeSystem { home: home.clone() });

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
