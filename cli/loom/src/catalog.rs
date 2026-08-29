use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Skill,
    PiPackage,
    HerdrPlugin,
    Tool,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skill => write!(formatter, "Skill"),
            Self::PiPackage => write!(formatter, "Pi package"),
            Self::HerdrPlugin => write!(formatter, "Herdr plugin"),
            Self::Tool => write!(formatter, "Tool"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: String,
    pub kind: ResourceKind,
    pub group: String,
    pub label: String,
    pub description: String,
    pub install_target: String,
    pub next_action: String,
    /// Skills this resource invokes (skill resources only); installs pull
    /// them in transitively.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// The executable to probe on PATH (tool resources only).
    #[serde(default)]
    pub bin: Option<String>,
    /// Exact npm version pin (external Pi packages only).
    #[serde(default)]
    pub version: Option<String>,
    /// Exact non-npm Pi package source, currently a Git commit pin.
    #[serde(default)]
    pub source: Option<String>,
    /// Hidden by native Windows setup; available when Loom runs inside WSL.
    #[serde(default)]
    pub windows_wsl: bool,
    /// Per-OS variant keys installed alongside the primary (tool resources
    /// only); mise os filters decide which applies on each machine.
    #[serde(default)]
    pub companions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub schema_version: u32,
    pub resources: Vec<Resource>,
}

impl Resource {
    pub fn pi_install_spec(&self) -> String {
        self.source.clone().unwrap_or_else(|| match &self.version {
            Some(version) => format!("npm:{}@{version}", self.install_target),
            None => format!("npm:{}", self.install_target),
        })
    }
}

impl Catalog {
    pub fn embedded() -> Result<Self> {
        let catalog: Self = serde_json::from_str(include_str!("../setup-catalog.json"))
            .context("embedded setup catalog is invalid")?;
        anyhow::ensure!(
            catalog.schema_version == 1,
            "unsupported setup catalog schema"
        );
        Ok(catalog)
    }

    pub fn find(&self, ids: &[String]) -> Result<Vec<Resource>> {
        ids.iter()
            .map(|id| {
                self.resources
                    .iter()
                    .find(|resource| &resource.id == id)
                    .cloned()
                    .with_context(|| format!("unknown resource: {id}"))
            })
            .collect()
    }
}
