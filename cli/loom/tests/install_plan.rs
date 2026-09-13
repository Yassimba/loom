use loom::{
    build_install_plan as build_plan, expand_skill_dependencies, InstallPlan, Operation, Platform,
    PrerequisiteStatus, Resource, ResourceKind, SkillAgent, SkillDestination, SkillScope,
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

    assert!(plan.prerequisite_count() == 0);
    assert_eq!(
        plan.resources()
            .map(|step| step.operation.clone())
            .collect::<Vec<_>>(),
        vec![
            Operation::Skills {
                skills: vec!["tdd".into()],
                destination: skill_destination(),
            },
            Operation::PiPackage {
                spec: "npm:@yassimba/pi-fast@latest".into(),
                name: "@yassimba/pi-fast".into(),
                project: false
            },
            Operation::HerdrPlugin {
                source: "Yassimba/loom/plugins/herdr-jumplist".into(),
                name: "yassin.jumplist".into()
            },
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
        plan.resources().next().unwrap().operation,
        Operation::PiPackage {
            spec: "git:github.com/example/pi-example@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            name: "pi-example".into(),
            project: false
        }
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
        plan.resources().next().unwrap().operation,
        Operation::Skills {
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
        plan.prerequisites()
            .map(|step| step.operation.display())
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
        plan.prerequisites()
            .map(|step| step.operation.display())
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

    assert_eq!(plan.resources().count(), 1);
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

    assert_eq!(plan.prerequisite_count(), 1);
    let step = &plan.steps[0];
    assert_eq!(step.manager(), "mise");
    assert_eq!(
        step.operation,
        Operation::Tools {
            tools: vec![
                "gh".to_string(),
                "npm:@earendil-works/pi-coding-agent".to_string(),
            ],
        }
    );
    // The tool resource itself produces no separate resource step.
    assert_eq!(plan.resources().count(), 1);
    assert_eq!(plan.resources().next().unwrap().manager(), "pi");
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

    assert_eq!(plan.prerequisite_count(), 2);
    assert_eq!(plan.steps[0].manager(), "mise");
    assert!(matches!(
        plan.steps[0].operation,
        Operation::BootstrapMise(_)
    ));
    assert!(matches!(plan.steps[1].operation, Operation::Tools { .. }));
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
        plan.steps[0].operation,
        Operation::Tools {
            tools: vec![
                "github:mikeleppane/envx".to_string(),
                "cargo:envex".to_string(),
            ],
        }
    );
}

#[test]
fn rtk_configures_pi_when_pi_is_present_or_selected() {
    let rtk = resource(ResourceKind::Tool, "tool:rtk", loom::manifest::RTK_TOOL_KEY);
    let pi = resource(ResourceKind::Tool, "tool:pi", loom::manifest::PI_TOOL_KEY);

    let with_installed_pi = build_install_plan(
        std::slice::from_ref(&rtk),
        PrerequisiteStatus {
            pi: true,
            herdr: false,
            mise: true,
        },
        Platform::Unix,
    )
    .unwrap();
    assert!(with_installed_pi
        .steps
        .iter()
        .any(|step| step.operation == Operation::RtkPi));

    let with_selected_pi = build_install_plan(
        &[rtk.clone(), pi],
        PrerequisiteStatus {
            pi: false,
            herdr: false,
            mise: true,
        },
        Platform::Unix,
    )
    .unwrap();
    assert!(with_selected_pi
        .steps
        .iter()
        .any(|step| step.operation == Operation::RtkPi));

    let without_pi = build_install_plan(
        &[rtk],
        PrerequisiteStatus {
            pi: false,
            herdr: false,
            mise: true,
        },
        Platform::Unix,
    )
    .unwrap();
    assert!(!without_pi
        .steps
        .iter()
        .any(|step| step.operation == Operation::RtkPi));
}

#[test]
fn local_mcp_servers_pull_exact_tools_and_pi_gateway() {
    let root = std::env::temp_dir().join(format!(
        "loom-local-mcp-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    let home = home.canonicalize().unwrap();
    let project = project.canonicalize().unwrap();
    let catalog = loom::Catalog::embedded().unwrap();
    for (name, tool_id, install_target) in [
        ("serena", "tool:serena", "pipx:serena-agent"),
        (
            "codebase-memory-mcp",
            "tool:codebase-memory-mcp",
            "npm:codebase-memory-mcp",
        ),
    ] {
        let selected = catalog.find(&[format!("mcp-server:{name}")]).unwrap();
        let expanded = expand_skill_dependencies(&catalog.resources, selected, &[SkillAgent::Pi]);
        assert!(expanded.iter().any(|resource| resource.id == tool_id));
        assert!(expanded
            .iter()
            .any(|resource| resource.id == "pi-package:pi-mcp-adapter"));
        assert!(expanded
            .iter()
            .any(|resource| { resource.id == "pi-package:@yassimba/pi-code-intelligence" }));

        let pi_destination =
            SkillDestination::new(vec![SkillAgent::Pi], SkillScope::Global, &home, &project);
        let plan = build_plan(
            &expanded,
            PrerequisiteStatus {
                pi: true,
                herdr: false,
                mise: true,
            },
            Platform::Unix,
            &pi_destination,
        )
        .unwrap();
        assert!(plan.prerequisites().any(|step| {
            matches!(&step.operation, Operation::Tools { tools } if tools.contains(&install_target.to_string()))
        }));
        assert!(plan
            .resources()
            .any(|step| step.target == format!("mcp-server:{name}")));

        let destination = SkillDestination::new(
            vec![SkillAgent::Claude],
            SkillScope::Global,
            &home,
            &project,
        );
        assert!(build_plan(
            &expanded,
            PrerequisiteStatus {
                pi: false,
                herdr: false,
                mise: true,
            },
            Platform::Unix,
            &destination,
        )
        .unwrap_err()
        .to_string()
        .contains("Pi selected"));
    }
    std::fs::remove_dir_all(root).unwrap();
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
