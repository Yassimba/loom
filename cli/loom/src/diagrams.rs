//! Diagram output preferences; atlas research and model selection are independent.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{fmt, fs, path::Path};

pub const PROJECT_PATH: &str = "ai-docs/agents/diagrams.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum DiagramStyle {
    Inherit,
    Polished,
    Economical,
}

impl DiagramStyle {
    pub fn value(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::Polished => "polished",
            Self::Economical => "economical",
        }
    }
}

impl fmt::Display for DiagramStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inherit => "Use my setup default",
            Self::Polished => "Polished — custom diagram layouts",
            Self::Economical => "Economical — simpler Mermaid layouts",
        })
    }
}

/// Update only our field. Malformed existing configuration is never overwritten.
pub fn write_style(path: &Path, style: DiagramStyle) -> Result<bool> {
    let mut value: Value = match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .with_context(|| format!("Could not save diagram style: {} contains invalid JSON. Fix the JSON and retry; the file was left unchanged", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()))
        }
    };
    let object = value
        .as_object_mut()
        .with_context(|| format!("Could not save diagram style: {} must contain a JSON object, for example {{\"style\":\"polished\"}}. The file was left unchanged", path.display()))?;
    if object.get("style") == Some(&json!(style.value())) {
        return Ok(false);
    }
    object.insert("style".into(), json!(style.value()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(&value)?))
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_switch_idempotently_and_preserve_other_content() {
        let root = std::env::temp_dir().join(format!("loom-diagrams-{}", std::process::id()));
        let path = root.join("diagrams.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, "{\"other\": 42}").unwrap();
        for style in [
            DiagramStyle::Economical,
            DiagramStyle::Polished,
            DiagramStyle::Inherit,
        ] {
            assert!(write_style(&path, style).unwrap());
            assert!(!write_style(&path, style).unwrap());
            let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(value, json!({"other": 42, "style": style.value()}));
        }
        for invalid in ["broken", "[]", "null"] {
            fs::write(&path, invalid).unwrap();
            assert!(write_style(&path, DiagramStyle::Polished).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
