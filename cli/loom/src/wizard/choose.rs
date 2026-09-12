//! Choose-stage group construction: profiles, kinds, and resource lists.
use super::state::{Group, Item, KindGroup, Model, WizardPurpose};
use crate::{Resource, ResourceKind};

/// The install chooser starts with overlapping role profiles. Uninstall and
/// profile-less fixtures keep the ownership and resource-kind groups.
pub(super) fn choose_groups(model: &Model) -> Vec<Group> {
    if model.purpose == WizardPurpose::Install && !model.profiles.is_empty() {
        return profile_groups(model);
    }

    let mut groups = resource_groups(model);
    let mut seen: Vec<&str> = Vec::new();
    for spec in &model.settings {
        if seen.contains(&spec.group.as_str()) {
            continue;
        }
        seen.push(&spec.group);
        let rows = model
            .settings
            .iter()
            .enumerate()
            .filter(|(_, other)| other.group == spec.group)
            .map(|(index, _)| Item::Setting(index))
            .collect::<Vec<_>>();
        groups.push(Group {
            title: format!("Settings · {}", spec.group),
            description: spec.description.clone(),
            bulk_rows: rows.clone(),
            kinds: vec![KindGroup {
                title: "Settings".into(),
                rows,
            }],
            everything: false,
        });
    }
    groups
}

fn profile_groups(model: &Model) -> Vec<Group> {
    let all_resources = (0..model.resources.len())
        .filter(|index| {
            !model.resources[*index].is_automatic_pi_package()
                && model.resources[*index].group != "Wiki"
        })
        .map(Item::Resource)
        .collect::<Vec<_>>();
    let mut groups = Vec::new();
    for profile in &model.profiles {
        let members = if profile.id == "knowledge-wiki" {
            model
                .resources
                .iter()
                .filter(|resource| resource.group == "Wiki")
                .map(|resource| resource.id.clone())
                .collect()
        } else {
            profile.resources.clone()
        };
        let direct = members
            .iter()
            .filter_map(|id| {
                model
                    .resources
                    .iter()
                    .position(|resource| &resource.id == id)
            })
            .filter(|index| {
                !model.resources[*index].is_automatic_pi_package()
                    && (profile.id == "knowledge-wiki") == (model.resources[*index].group == "Wiki")
            })
            .map(Item::Resource)
            .collect::<Vec<_>>();
        if direct.is_empty() {
            continue;
        }
        let selected = direct
            .iter()
            .filter_map(|row| match row {
                Item::Resource(index) => Some(model.resources[*index].clone()),
                Item::Setting(_) => None,
            })
            .collect();
        let rows = crate::expand_skill_dependencies(
            &model.resources,
            selected,
            &model.skill_destination.agents,
        )
        .iter()
        .filter(|resource| (profile.id == "knowledge-wiki") == (resource.group == "Wiki"))
        .filter_map(|resource| {
            model
                .resources
                .iter()
                .position(|candidate| candidate.id == resource.id)
                .map(Item::Resource)
        })
        .collect::<Vec<_>>();
        let kinds = profile_kinds(model, &rows);
        groups.push(Group {
            title: profile.label.clone(),
            description: profile.description.clone(),
            bulk_rows: direct,
            kinds,
            everything: false,
        });
    }
    let kinds = profile_kinds(model, &all_resources);
    groups.push(Group {
        title: "Everything".into(),
        description: "Every general capability. Vault-scoped resources are under Wiki.".into(),
        bulk_rows: all_resources,
        kinds,
        everything: true,
    });
    groups
}

fn profile_kinds(model: &Model, rows: &[Item]) -> Vec<KindGroup> {
    let mut kinds = Vec::new();
    for (kind, title) in [
        (ResourceKind::Skill, "Skills"),
        (ResourceKind::Tool, "Tools"),
        (ResourceKind::PiPackage, "Pi packages"),
        (ResourceKind::HerdrPlugin, "Herdr plugins"),
        (ResourceKind::McpServer, "MCP servers"),
    ] {
        let visible = rows
            .iter()
            .filter(
                |row| matches!(row, Item::Resource(index) if model.resources[*index].kind == kind),
            )
            .cloned()
            .collect::<Vec<_>>();
        if visible.is_empty() {
            continue;
        }
        kinds.push(KindGroup {
            title: title.into(),
            rows: visible,
        });
    }
    if !model.settings.is_empty()
        && !rows.iter().any(
            |row| matches!(row, Item::Resource(index) if model.resources[*index].group == "Wiki"),
        )
    {
        let settings = (0..model.settings.len())
            .map(Item::Setting)
            .collect::<Vec<_>>();
        kinds.push(KindGroup {
            title: "Settings".into(),
            rows: settings,
        });
    }
    kinds
}

fn resource_groups(model: &Model) -> Vec<Group> {
    let mut groups = Vec::new();
    let visible = |index: usize| {
        model.purpose == WizardPurpose::Uninstall
            || !model.resources[index].is_automatic_pi_package()
    };
    let rows = (0..model.resources.len())
        .filter(|index| visible(*index) && model.resources[*index].group != "Wiki")
        .map(Item::Resource)
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        groups.push(Group {
            title: "Everything".into(),
            description: "Every general resource. Vault-scoped resources are under Wiki.".into(),
            bulk_rows: rows.clone(),
            kinds: vec![KindGroup {
                title: "Items".into(),
                rows,
            }],
            everything: true,
        });
    }
    let mut push_group = |title: String, items: Vec<usize>| {
        let rows = items
            .into_iter()
            .filter(|index| visible(*index))
            .map(Item::Resource)
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            groups.push(Group {
                description: title.clone(),
                title,
                bulk_rows: rows.clone(),
                kinds: vec![KindGroup {
                    title: "Items".into(),
                    rows,
                }],
                everything: false,
            });
        }
    };
    push_group(
        "Wiki".into(),
        indices(&model.resources, |resource| resource.group == "Wiki"),
    );
    for category in groups_of(&model.resources, ResourceKind::Skill)
        .into_iter()
        .filter(|category| category != "Wiki")
    {
        let items = indices(&model.resources, |resource| {
            resource.kind == ResourceKind::Skill && resource.group == category
        });
        push_group(format!("Skills · {category}"), items);
    }
    for (kind, title) in [
        (ResourceKind::Tool, "Tools"),
        (ResourceKind::PiPackage, "Pi packages"),
        (ResourceKind::HerdrPlugin, "Herdr plugins"),
        (ResourceKind::McpServer, "MCP servers"),
    ] {
        push_group(
            title.into(),
            indices(&model.resources, |resource| {
                resource.kind == kind && resource.group != "Wiki"
            }),
        );
    }
    groups
}

fn groups_of(resources: &[Resource], kind: ResourceKind) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for resource in resources.iter().filter(|resource| resource.kind == kind) {
        if !groups.contains(&resource.group) {
            groups.push(resource.group.clone());
        }
    }
    groups
}

fn indices(resources: &[Resource], keep: impl Fn(&Resource) -> bool) -> Vec<usize> {
    resources
        .iter()
        .enumerate()
        .filter(|(_, resource)| keep(resource))
        .map(|(index, _)| index)
        .collect()
}
