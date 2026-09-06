use loom::{
    build_install_plan as build_plan, expand_skill_dependencies, CommandSpec, InstallPlan,
    Platform, PrerequisiteStatus, Resource, ResourceKind, SkillAgent, SkillDestination, SkillScope,
    StepAction, VerificationSpec,
};
use pretty_assertions::assert_eq;

fn skill_destination() -> SkillDestination {
    SkillDestination::new(
        SkillAgent::ALL.to_vec(),
        SkillScope::Global,
        std::path::Path::new("/tmp/loom-test-home"),
        std::path::Path::new("/tmp/loom-test-project"),
    )
}

fn build_install_plan(
    resources: &[Resource],
    status: PrerequisiteStatus,
    platform: Platform,
) -> anyhow::Result<InstallPlan> {
    build_plan(resources, status, platform, &skill_destination())
}

fn resource(kind: ResourceKind, id: &str, target: &str) -> Resource {
    Resource {
        id: id.into(),
        kind,
        group: "Test".into(),
        label: id.into(),
        description: "Test resource".into(),
        install_target: target.into(),
        next_action: "Try it".into(),
        dependencies: Vec::new(),
        bin: None,
        version: None,
        source: None,
        windows_wsl: false,
        companions: Vec::new(),
        bundled_skills: Vec::new(),
    }
}

fn skill_with_deps(id: &str, target: &str, dependencies: &[&str]) -> Resource {
    Resource {
        dependencies: dependencies.iter().map(ToString::to_string).collect(),
        ..resource(ResourceKind::Skill, id, target)
    }
}

#[test]
fn sem_mcp_requires_a_verified_destination() {
    let error = build_install_plan(
        &[resource(ResourceKind::McpServer, "mcp-server:sem", "sem")],
        PrerequisiteStatus {
            pi: false,
            herdr: false,
            mise: false,
        },
        Platform::Unix,
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("other agent adapters are not yet verified"));
}

#[test]
fn mixed_selection_copies_skills_and_delegates_the_rest() {
    let resources = vec![
        resource(ResourceKind::Skill, "skill:tdd", "tdd"),
        resource(
            ResourceKind::PiPackage,
            "pi-package:@yassimba/pi-fast",
            "@yassimba/pi-fast",
        ),
        resource(
            ResourceKind::HerdrPlugin,
            "herdr-plugin:yassin.jumplist",
            "Yassimba/loom/plugins/herdr-jumplist",
        ),
    ];
    let status = PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: false,
    };

    let plan = build_install_plan(&resources, status, Platform::Unix).unwrap();

    assert!(plan.prerequisites.is_empty());
    assert_eq!(
        plan.resources
            .iter()
            .map(|step| step.action.clone())
            .collect::<Vec<_>>(),
        vec![
            StepAction::CopySkills {
                skills: vec!["tdd".into()],
                destination: skill_destination(),
            },
            StepAction::Command(CommandSpec::new(
                "pi",
                ["install", "npm:@yassimba/pi-fast@latest"],
            )),
            StepAction::Command(CommandSpec::new(
                "herdr",
                [
                    "plugin",
                    "install",
                    "Yassimba/loom/plugins/herdr-jumplist",
                    "--yes",
                ],
            )),
        ]
    );
    assert_eq!(
        plan.resources
            .iter()
            .map(|step| step.verification.clone())
            .collect::<Vec<_>>(),
        vec![
            // Skills are verified inside the copy: each tree must end up
            // with <skill>/SKILL.md.
            None,
            Some(VerificationSpec {
                command: CommandSpec::new("pi", ["list"]),
                needle: Some("@yassimba/pi-fast".into()),
            }),
            Some(VerificationSpec {
                command: CommandSpec::new("herdr", ["plugin", "list"]),
                needle: Some("yassin.jumplist".into()),
            }),
        ]
    );
}

#[test]
fn git_pi_package_uses_its_exact_source() {
    let mut example = resource(
        ResourceKind::PiPackage,
        "pi-package:pi-example",
        "pi-example",
    );
    example.source =
        Some("git:github.com/example/pi-example@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    let status = PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: false,
    };

    let plan = build_install_plan(&[example], status, Platform::Unix).unwrap();

    assert_eq!(
        plan.resources[0].action,
        StepAction::Command(CommandSpec::new(
            "pi",
            [
                "install",
                "git:github.com/example/pi-example@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ],
        ))
    );
}

#[test]
fn skill_selection_expands_to_its_dependency_closure() {
    let catalog = vec![
        skill_with_deps("skill:release", "release", &["commit"]),
        skill_with_deps("skill:commit", "commit", &["write-simply"]),
        skill_with_deps("skill:write-simply", "write-simply", &[]),
        skill_with_deps("skill:unrelated", "unrelated", &[]),
    ];

    let expanded = expand_skill_dependencies(&catalog, vec![catalog[0].clone()], &[]);
    let plan = build_install_plan(
        &expanded,
        PrerequisiteStatus {
            pi: true,
            herdr: true,
            mise: false,
        },
        Platform::Unix,
    )
    .unwrap();

    assert_eq!(
        plan.resources[0].action,
        StepAction::CopySkills {
            skills: vec!["release".into(), "commit".into(), "write-simply".into(),],
            destination: skill_destination(),
        }
    );
}

#[test]
fn missing_foundations_are_installed_before_selected_resources() {
    let resources = vec![
        resource(ResourceKind::Skill, "skill:tdd", "tdd"),
        resource(
            ResourceKind::HerdrPlugin,
            "herdr-plugin:yassin.jumplist",
            "Yassimba/loom/plugins/herdr-jumplist",
        ),
    ];
    let status = PrerequisiteStatus {
        pi: true,
        herdr: false,
        mise: false,
    };

    let plan = build_install_plan(&resources, status, Platform::Windows).unwrap();

    assert_eq!(
        plan.prerequisites
            .iter()
            .map(|step| step.action.display())
            .collect::<Vec<_>>(),
        vec![
            "powershell -NoProfile -ExecutionPolicy Bypass -Command winget install --id jdx.mise --silent --accept-package-agreements --accept-source-agreements",
            "add to the mise selection and install: herdr",
        ]
    );
}

#[test]
fn selecting_a_pi_package_without_pi_uses_the_pinned_mise_runtime() {
    let resources = vec![resource(
        ResourceKind::PiPackage,
        "pi-package:@yassimba/pi-fast",
        "@yassimba/pi-fast",
    )];
    let status = PrerequisiteStatus {
        pi: false,
        herdr: true,
        mise: false,
    };

    let plan = build_install_plan(&resources, status, Platform::Unix).unwrap();

    assert_eq!(
        plan.prerequisites
            .iter()
            .map(|step| step.action.display())
            .collect::<Vec<_>>(),
        vec![
            "sh -c curl -fsSL https://mise.run | sh",
            "add to the mise selection and install: npm:@earendil-works/pi-coding-agent",
        ]
    );
}

#[test]
fn an_installed_pi_needs_no_runtime_install() {
    let resources = vec![resource(
        ResourceKind::PiPackage,
        "pi-package:@yassimba/pi-fast",
        "@yassimba/pi-fast",
    )];
    let status = PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: false,
    };

    let plan = build_install_plan(&resources, status, Platform::Unix).unwrap();

    assert_eq!(plan.resources.len(), 1);
}

#[test]
fn selected_tools_sync_through_mise_before_resources() {
    let resources = vec![
        resource(ResourceKind::Tool, "tool:gh", "gh"),
        resource(
            ResourceKind::PiPackage,
            "pi-package:@yassimba/pi-fast",
            "@yassimba/pi-fast",
        ),
    ];
    // Pi is missing but mise is present: Pi rides along as a manifest tool
    // instead of a global npm install.
    let status = PrerequisiteStatus {
        pi: false,
        herdr: true,
        mise: true,
    };

    let plan = build_install_plan(&resources, status, Platform::Unix).unwrap();

    assert_eq!(plan.prerequisites.len(), 1);
    let step = &plan.prerequisites[0];
    assert_eq!(step.manager, "mise");
    assert_eq!(
        step.action,
        StepAction::SyncTools {
            tools: vec![
                "gh".to_string(),
                "npm:@earendil-works/pi-coding-agent".to_string(),
            ],
        }
    );
    // The tool resource itself produces no separate resource step.
    assert_eq!(plan.resources.len(), 1);
    assert_eq!(plan.resources[0].manager, "pi");
}

#[test]
fn tools_without_mise_get_a_mise_prerequisite() {
    let resources = vec![resource(ResourceKind::Tool, "tool:gh", "gh")];
    let status = PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: false,
    };

    let plan = build_install_plan(&resources, status, Platform::Unix).unwrap();

    assert_eq!(plan.prerequisites.len(), 2);
    assert_eq!(plan.prerequisites[0].manager, "mise");
    assert!(matches!(
        plan.prerequisites[0].action,
        StepAction::Command(_)
    ));
    assert!(matches!(
        plan.prerequisites[1].action,
        StepAction::SyncTools { .. }
    ));
}

#[test]
fn tool_companions_join_the_mise_sync() {
    let mut envx = resource(ResourceKind::Tool, "tool:envx", "github:mikeleppane/envx");
    envx.companions = vec!["cargo:envex".to_string()];
    let status = PrerequisiteStatus {
        pi: true,
        herdr: true,
        mise: true,
    };

    let plan = build_install_plan(&[envx], status, Platform::Unix).unwrap();

    assert_eq!(
        plan.prerequisites[0].action,
        StepAction::SyncTools {
            tools: vec![
                "github:mikeleppane/envx".to_string(),
                "cargo:envex".to_string(),
            ],
        }
    );
}

#[test]
fn bundled_package_auto_selects_its_shared_skill() {
    let catalog = loom::Catalog::embedded().unwrap();
    for id in [
        "pi-package:i-have-adhd",
        "pi-package:@dietrichgebert/ponytail",
    ] {
        let package = catalog.find(&[id.into()]).unwrap();
        let name = package[0].label.clone();
        let expanded = expand_skill_dependencies(&catalog.resources, package, &[SkillAgent::Pi]);
        assert!(expanded
            .iter()
            .any(|resource| resource.id == format!("skill:{name}")));
    }
}
