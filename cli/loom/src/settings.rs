//! Curated tool-settings toggles offered by the setup wizard. Specs live in
//! `settings.json`; this module applies them with format-preserving edits —
//! `toml_edit` for Herdr, JSONC splicing for Zed.

use crate::jsonc;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct KeyCommand {
    pub key: String,
    pub kind: String,
    pub command: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ZedKeybinding {
    pub key: String,
    pub action: Json,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SettingChange {
    /// Append `[[keys.command]]` bindings to Herdr's config unless a binding
    /// for the same command already exists.
    HerdrKeyCommands { commands: Vec<KeyCommand> },
    /// Set (or subset-merge into) a top-level key in Zed's settings.json.
    ZedValue { key: String, value: Json },
    /// Bind keys in one `context` block of Zed's keymap.json by appending a
    /// block at the end (a later block wins in Zed). A key the user already
    /// bound anywhere in that context is left alone.
    ZedKeymap {
        context: String,
        bindings: Vec<ZedKeybinding>,
    },
    /// Create a JSON config with curated defaults, but never replace an
    /// existing file.
    PiFffDefaults { value: Json },
    /// Create the upstream Pi ADHD plugin's always-on flag without replacing it.
    PiAdhdAlwaysOn,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingSpec {
    pub id: String,
    pub group: String,
    pub label: String,
    pub description: String,
    /// Catalog resource whose selection should pre-check this setting.
    #[serde(default)]
    pub related_resource: Option<String>,
    pub change: SettingChange,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDocument {
    curated: Vec<SettingSpec>,
    responses: SettingResponses,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingResponses {
    pi_adhd: SettingSpec,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingState {
    Applied,
    NotApplied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsPaths {
    pub herdr_config: PathBuf,
    pub zed_settings: PathBuf,
    pub zed_keymap: PathBuf,
    pub pi_fff_config: PathBuf,
    pub pi_adhd_flag: PathBuf,
}

impl SettingsPaths {
    pub fn detect() -> Result<Self> {
        let xdg = xdg_config_home();
        let home = dirs::home_dir().context("home directory is unavailable")?;
        // Zed honors XDG_CONFIG_HOME only on Linux; Windows is always
        // %APPDATA%\Zed and macOS is always ~/.config/zed.
        let zed_dir = if cfg!(windows) {
            native_config_dir()?.join("Zed")
        } else if cfg!(target_os = "macos") {
            home_config_dir()?.join("zed")
        } else {
            match &xdg {
                Some(base) => base.join("zed"),
                None => home_config_dir()?.join("zed"),
            }
        };
        let pi_agent_dir = pi_agent_dir(&home);
        Ok(Self {
            herdr_config: herdr_dir(&home).join("config.toml"),
            zed_settings: zed_dir.join("settings.json"),
            zed_keymap: zed_dir.join("keymap.json"),
            pi_fff_config: pi_agent_dir.join("pi-fff.json"),
            pi_adhd_flag: pi_agent_dir.join(".i-have-adhd-always"),
        })
    }
}

fn xdg_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Directory Herdr actually reads: `$XDG_CONFIG_HOME/herdr` on every OS, else
/// `%APPDATA%\herdr` on Windows and `~/.config/herdr` on Unix. `home` is the
/// profile root so tests never touch the real AppData tree.
pub(crate) fn herdr_dir(home: &Path) -> PathBuf {
    let base = match xdg_config_home() {
        Some(base) => base,
        None if cfg!(windows) => {
            if dirs::home_dir().as_deref() == Some(home) {
                native_config_dir().unwrap_or_else(|_| home.join("AppData").join("Roaming"))
            } else {
                home.join("AppData").join("Roaming")
            }
        }
        None => home.join(".config"),
    };
    base.join("herdr")
}

pub(crate) fn pi_agent_dir(home: &Path) -> PathBuf {
    let directory = std::env::var_os("PI_CODING_AGENT_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pi/agent"));
    // Pi expands a leading ~ even when the shell leaves it quoted.
    match directory.strip_prefix("~") {
        Ok(relative) => home.join(relative),
        Err(_) => directory,
    }
}

fn native_config_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        return dirs::config_dir()
            .or_else(|| {
                std::env::var_os("APPDATA")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| dirs::home_dir().map(|home| home.join("AppData").join("Roaming")))
            .context("config directory is unavailable");
    }
    home_config_dir()
}

fn home_config_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("home directory is unavailable")?
        .join(".config"))
}

fn settings_document() -> &'static SettingsDocument {
    static DOCUMENT: OnceLock<SettingsDocument> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        serde_json::from_str(include_str!("../settings.json"))
            .expect("embedded settings.json is invalid")
    })
}

/// Offered only through the setup question, never through bulk setting selection.
pub fn pi_adhd_setting() -> SettingSpec {
    settings_document().responses.pi_adhd.clone()
}

pub fn curated_settings() -> Vec<SettingSpec> {
    settings_document().curated.clone()
}

impl SettingSpec {
    pub fn target_path<'a>(&self, paths: &'a SettingsPaths) -> &'a Path {
        match self.change {
            SettingChange::HerdrKeyCommands { .. } => &paths.herdr_config,
            SettingChange::ZedValue { .. } => &paths.zed_settings,
            SettingChange::ZedKeymap { .. } => &paths.zed_keymap,
            SettingChange::PiFffDefaults { .. } => &paths.pi_fff_config,
            SettingChange::PiAdhdAlwaysOn => &paths.pi_adhd_flag,
        }
    }

    /// Whether the change writes to a Zed config file — such a setting only
    /// makes sense on a machine that has Zed at all.
    pub fn requires_zed(&self) -> bool {
        matches!(
            self.change,
            SettingChange::ZedValue { .. } | SettingChange::ZedKeymap { .. }
        )
    }

    /// A short, review-screen friendly rendition of what gets written.
    pub fn change_summary(&self) -> Vec<String> {
        match &self.change {
            SettingChange::PiAdhdAlwaysOn => vec!["Enable ADHD-friendly responses for future Pi sessions; leave existing settings unchanged".into()],
            SettingChange::HerdrKeyCommands { commands } => commands
                .iter()
                .map(|command| format!("[[keys.command]] {} → {}", command.key, command.command))
                .collect(),
            SettingChange::ZedValue { key, value } => vec![format!("\"{key}\": {value}")],
            SettingChange::ZedKeymap { context, bindings } => bindings
                .iter()
                .map(|binding| format!("{context}: \"{}\" → {}", binding.key, binding.action))
                .collect(),
            SettingChange::PiFffDefaults { .. } => {
                vec!["create override config if missing".into()]
            }
        }
    }
}

pub fn setting_state(spec: &SettingSpec, paths: &SettingsPaths) -> SettingState {
    if matches!(spec.change, SettingChange::PiAdhdAlwaysOn) {
        return if spec.target_path(paths).exists() {
            SettingState::Applied
        } else {
            SettingState::NotApplied
        };
    }
    let content = match fs::read_to_string(spec.target_path(paths)) {
        Ok(content) => content,
        Err(_) => return SettingState::NotApplied,
    };
    let applied = match &spec.change {
        SettingChange::HerdrKeyCommands { commands } => {
            apply_herdr_bindings(&content, commands).is_ok_and(|change| change.is_none())
        }
        SettingChange::ZedValue { key, value } => {
            apply_zed_value(&content, key, value).is_ok_and(|change| change.is_none())
        }
        SettingChange::ZedKeymap { context, bindings } => jsonc::parse_document(&content)
            .map(|document| {
                bindings
                    .iter()
                    .all(|binding| zed_keymap_binds(&document, context, &binding.key))
            })
            .unwrap_or(false),
        SettingChange::PiFffDefaults { .. } => true,
        SettingChange::PiAdhdAlwaysOn => unreachable!(),
    };
    if applied {
        SettingState::Applied
    } else {
        SettingState::NotApplied
    }
}

/// Apply the setting; returns false when the file already had it.
pub fn apply_setting(spec: &SettingSpec, paths: &SettingsPaths) -> Result<bool> {
    let path = spec.target_path(paths);
    if matches!(
        spec.change,
        SettingChange::PiAdhdAlwaysOn | SettingChange::PiFffDefaults { .. }
    ) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let mut file = match fs::File::options().write(true).create_new(true).open(path) {
            Ok(file) => file,
            Err(error)
                if error.kind() == std::io::ErrorKind::AlreadyExists
                    && (matches!(spec.change, SettingChange::PiFffDefaults { .. })
                        || path.exists()) =>
            {
                return Ok(false)
            }
            Err(error) => {
                return Err(error).with_context(|| format!("could not create {}", path.display()))
            }
        };
        if let SettingChange::PiFffDefaults { value: defaults } = &spec.change {
            let content = format!("{}\n", serde_json::to_string_pretty(defaults)?);
            file.write_all(content.as_bytes())
                .with_context(|| format!("could not write {}", path.display()))?;
        }
        return Ok(true);
    }
    let existing = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match &spec.change {
            SettingChange::HerdrKeyCommands { .. } => String::new(),
            SettingChange::ZedValue { .. } => "{}\n".into(),
            SettingChange::ZedKeymap { .. } => "[]\n".into(),
            SettingChange::PiFffDefaults { .. } | SettingChange::PiAdhdAlwaysOn => unreachable!(),
        },
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()))
        }
    };
    let updated = match &spec.change {
        SettingChange::HerdrKeyCommands { commands } => apply_herdr_bindings(&existing, commands)?,
        SettingChange::ZedValue { key, value } => apply_zed_value(&existing, key, value)?,
        SettingChange::ZedKeymap { context, bindings } => {
            apply_zed_keymap(&existing, context, bindings)?
        }
        SettingChange::PiFffDefaults { .. } | SettingChange::PiAdhdAlwaysOn => unreachable!(),
    };
    let Some(updated) = updated else {
        return Ok(false);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(path, updated).with_context(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

fn herdr_has_binding(document: &DocumentMut, command: &str) -> bool {
    document
        .get("keys")
        .and_then(Item::as_table)
        .and_then(|keys| keys.get("command"))
        .and_then(Item::as_array_of_tables)
        .is_some_and(|bindings| {
            bindings
                .iter()
                .any(|binding| binding.get("command").and_then(Item::as_str) == Some(command))
        })
}

pub fn apply_herdr_bindings(content: &str, commands: &[KeyCommand]) -> Result<Option<String>> {
    let mut document: DocumentMut = content
        .parse()
        .context("Herdr config.toml could not be parsed")?;
    let missing = commands
        .iter()
        .filter(|command| !herdr_has_binding(&document, &command.command))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(None);
    }
    let keys = document
        .entry("keys")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_mut()
        .context("Herdr config has a non-table [keys] entry")?;
    let bindings = keys
        .entry("command")
        .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .context("Herdr config has a non-array keys.command entry")?;
    for command in missing {
        let mut binding = Table::new();
        binding["key"] = toml_edit::value(command.key);
        binding["type"] = toml_edit::value(command.kind);
        binding["command"] = toml_edit::value(command.command);
        if let Some(description) = command.description {
            binding["description"] = toml_edit::value(description);
        }
        bindings.push(binding);
    }
    Ok(Some(document.to_string()))
}

/// True when any block of the keymap with this `context` binds `key` — to
/// anything. A user's own binding for the key counts as applied, so setup
/// never overrides a deliberate choice.
fn zed_keymap_binds(document: &Json, context: &str, key: &str) -> bool {
    document.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            block.get("context").and_then(Json::as_str) == Some(context)
                && block
                    .get("bindings")
                    .and_then(Json::as_object)
                    .is_some_and(|bindings| bindings.contains_key(key))
        })
    })
}

pub fn apply_zed_keymap(
    content: &str,
    context: &str,
    bindings: &[ZedKeybinding],
) -> Result<Option<String>> {
    let document = jsonc::parse_document(content).unwrap_or_else(|_| Json::Array(Vec::new()));
    let missing = bindings
        .iter()
        .filter(|binding| !zed_keymap_binds(&document, context, &binding.key))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(None);
    }
    // Rendered by hand: serde would sort the keys ("bindings" before
    // "context") and the block should read the way Zed's docs write it.
    let mut block = String::from("{\n");
    block.push_str(&format!(
        "  \"context\": {},\n",
        Json::String(context.into())
    ));
    block.push_str("  \"bindings\": {\n");
    for binding in missing {
        block.push_str(&format!(
            "    {}: {},\n",
            Json::String(binding.key.clone()),
            binding.action
        ));
    }
    block.push_str("  }\n}");
    jsonc::push_root_array_item(content, &block).map(Some)
}

pub fn apply_zed_value(content: &str, key: &str, value: &Json) -> Result<Option<String>> {
    let current = jsonc::get(content, key);
    if current
        .as_ref()
        .is_some_and(|current| json_subset(value, current))
    {
        return Ok(None);
    }
    // Object values merge over what is already there so unrelated fields of
    // the same object survive; scalars replace.
    let merged = match (value, &current) {
        (Json::Object(wanted), Some(Json::Object(existing))) => {
            let mut merged = existing.clone();
            merged.extend(wanted.clone());
            Json::Object(merged)
        }
        _ => value.clone(),
    };
    jsonc::set(content, key, &merged).map(Some)
}

/// True when every field of `expected` is present in `actual` (recursively);
/// scalars compare by equality.
fn json_subset(expected: &Json, actual: &Json) -> bool {
    match (expected, actual) {
        (Json::Object(expected), Json::Object(actual)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual| json_subset(value, actual))
        }),
        _ => expected == actual || numbers_equal(expected, actual),
    }
}

fn numbers_equal(expected: &Json, actual: &Json) -> bool {
    match (expected.as_f64(), actual.as_f64()) {
        (Some(expected), Some(actual)) => expected == actual,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn reviewr_binding() -> Vec<KeyCommand> {
        vec![KeyCommand {
            key: "prefix+r".into(),
            kind: "plugin_action".into(),
            command: "yassimba.reviewr.toggle".into(),
            description: Some("Reviewr: toggle sidebar".into()),
        }]
    }

    #[test]
    fn settings_json_keeps_question_only_adhd_out_of_bulk_selection() {
        let curated = curated_settings();
        let adhd = pi_adhd_setting();
        assert!(curated.iter().all(|setting| setting.id != adhd.id));
        let mut ids: Vec<_> = curated.iter().map(|setting| setting.id.as_str()).collect();
        ids.push(adhd.id.as_str());
        let unique = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), unique);
        let catalog = crate::Catalog::embedded().unwrap();
        for setting in curated.iter().chain(std::iter::once(&adhd)) {
            if let Some(related) = &setting.related_resource {
                assert!(
                    catalog
                        .resources
                        .iter()
                        .any(|resource| resource.id == *related),
                    "unknown related resource {related}"
                );
            }
        }
        assert!(matches!(adhd.change, SettingChange::PiAdhdAlwaysOn));
    }

    #[test]
    fn annotate_setting_includes_full_plugin_bindings() {
        let setting = curated_settings()
            .into_iter()
            .find(|setting| setting.id == "herdr:annotate-keybindings")
            .unwrap();
        let SettingChange::HerdrKeyCommands { commands } = setting.change else {
            panic!("annotate setting must add Herdr key commands");
        };
        assert_eq!(commands.len(), 5);
        assert!(commands
            .iter()
            .any(|command| command.command == "annotate.open"));
        assert!(commands
            .iter()
            .any(|command| command.command == "annotate.last"));
    }

    #[test]
    fn herdr_binding_is_appended_once() {
        let base = "[theme]\nname = \"catppuccin\"\n\n[keys]\nprefix = \"alt+space\"\n";
        let updated = apply_herdr_bindings(base, &reviewr_binding())
            .unwrap()
            .expect("first apply changes the file");
        assert!(updated.contains("[[keys.command]]"));
        assert!(updated.contains("command = \"yassimba.reviewr.toggle\""));
        assert!(updated.contains("name = \"catppuccin\""));
        assert_eq!(
            apply_herdr_bindings(&updated, &reviewr_binding()).unwrap(),
            None
        );
    }

    #[test]
    fn herdr_binding_detection_matches_on_command_not_key() {
        let base = concat!(
            "[keys]\n",
            "[[keys.command]]\n",
            "key = \"prefix+x\"\n",
            "type = \"plugin_action\"\n",
            "command = \"yassimba.reviewr.toggle\"\n",
        );
        assert_eq!(
            apply_herdr_bindings(base, &reviewr_binding()).unwrap(),
            None
        );
    }

    #[test]
    fn herdr_bindings_create_the_keys_table_when_missing() {
        let updated = apply_herdr_bindings("", &reviewr_binding())
            .unwrap()
            .expect("apply changes the file");
        let document: DocumentMut = updated.parse().unwrap();
        assert!(herdr_has_binding(&document, "yassimba.reviewr.toggle"));
    }

    #[test]
    fn zed_value_preserves_comments_and_merges_objects() {
        let base = "// keep me\n{\n  \"centered_layout\": {\n    \"enabled\": true,\n  },\n}\n";
        let updated = apply_zed_value(
            base,
            "centered_layout",
            &json!({"left_padding": 0, "right_padding": 0}),
        )
        .unwrap()
        .expect("apply changes the file");
        assert!(updated.contains("// keep me"));
        assert_eq!(
            jsonc::get(&updated, "centered_layout"),
            Some(json!({"enabled": true, "left_padding": 0, "right_padding": 0}))
        );
    }

    #[test]
    fn zed_value_is_a_noop_when_already_a_subset() {
        let base = "{\n  \"zoomed_padding\": false,\n}\n";
        assert_eq!(
            apply_zed_value(base, "zoomed_padding", &json!(false)).unwrap(),
            None
        );
        let padded = "{\n  \"centered_layout\": { \"left_padding\": 0.0, \"right_padding\": 0, \"enabled\": true },\n}\n";
        assert_eq!(
            apply_zed_value(
                padded,
                "centered_layout",
                &json!({"left_padding": 0, "right_padding": 0})
            )
            .unwrap(),
            None
        );
    }

    fn history_keys() -> Vec<ZedKeybinding> {
        vec![
            ZedKeybinding {
                key: "cmd-left".into(),
                action: json!(["terminal::SendText", "\u{1b}[1;3D"]),
            },
            ZedKeybinding {
                key: "cmd-right".into(),
                action: json!(["terminal::SendText", "\u{1b}[1;3C"]),
            },
        ]
    }

    #[test]
    fn zed_keymap_appends_a_terminal_block_and_preserves_comments() {
        let base = "// keep me\n[\n  {\n    \"context\": \"Editor\",\n    \"bindings\": {\n      \"cmd-left\": \"pane::GoBack\"\n    }\n  }\n]\n";
        let updated = apply_zed_keymap(base, "Terminal", &history_keys())
            .unwrap()
            .expect("first apply changes the file");
        assert!(updated.contains("// keep me"));
        assert!(
            updated.contains("\\u001b[1;3D"),
            "the escape byte is written as a JSON escape, not raw: {updated}"
        );
        let document = jsonc::parse_document(&updated).unwrap();
        assert!(zed_keymap_binds(&document, "Terminal", "cmd-left"));
        assert!(zed_keymap_binds(&document, "Terminal", "cmd-right"));
        // The Editor block's own cmd-left is untouched and distinct.
        assert_eq!(
            document[0]["bindings"]["cmd-left"],
            json!("pane::GoBack"),
            "existing contexts are never edited"
        );
        assert_eq!(
            apply_zed_keymap(&updated, "Terminal", &history_keys()).unwrap(),
            None,
            "second apply is a no-op"
        );
    }

    #[test]
    fn zed_keymap_respects_a_users_own_binding_for_one_of_the_keys() {
        let base = "[\n  {\n    \"context\": \"Terminal\",\n    \"bindings\": {\n      \"cmd-left\": \"something::Custom\"\n    }\n  }\n]\n";
        let updated = apply_zed_keymap(base, "Terminal", &history_keys())
            .unwrap()
            .expect("the unbound key still gets added");
        let document = jsonc::parse_document(&updated).unwrap();
        assert_eq!(
            document[0]["bindings"]["cmd-left"],
            json!("something::Custom"),
            "the user's own cmd-left binding survives"
        );
        assert!(zed_keymap_binds(&document, "Terminal", "cmd-right"));
    }

    #[test]
    fn zed_keymap_builds_the_file_from_nothing() {
        let updated = apply_zed_keymap("[]\n", "Terminal", &history_keys())
            .unwrap()
            .expect("apply changes the file");
        let document = jsonc::parse_document(&updated).unwrap();
        assert!(zed_keymap_binds(&document, "Terminal", "cmd-left"));
    }

    #[test]
    fn adhd_flag_follows_the_pi_agent_directory() {
        const MARKER: &str = "LOOM_TEST_ADHD_PATH";
        if let Some(expected) = std::env::var_os(MARKER) {
            let paths = SettingsPaths::detect().unwrap();
            assert_eq!(
                paths.pi_adhd_flag,
                PathBuf::from(expected).join(".i-have-adhd-always")
            );
            return;
        }
        let home = dirs::home_dir().unwrap();
        let absolute = std::env::temp_dir().join("custom pi agent");
        let mut cases = vec![
            (None, home.join(".pi/agent")),
            (Some(PathBuf::new()), home.join(".pi/agent")),
            (Some(absolute.clone()), absolute),
            (Some(PathBuf::from("~/custom-pi")), home.join("custom-pi")),
            (Some(PathBuf::from("~")), home.clone()),
            (
                Some(PathBuf::from("relative-pi")),
                PathBuf::from("relative-pi"),
            ),
        ];
        if cfg!(windows) {
            cases.push((Some(PathBuf::from("~\\custom-pi")), home.join("custom-pi")));
        }
        for (override_dir, expected) in cases {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "settings::tests::adhd_flag_follows_the_pi_agent_directory",
                ])
                .env(MARKER, expected)
                .env_remove("PI_CODING_AGENT_DIR");
            if let Some(override_dir) = override_dir {
                command.env("PI_CODING_AGENT_DIR", override_dir);
            }
            assert!(command.status().unwrap().success());
        }
    }

    #[test]
    fn pi_defaults_create_once_and_preserve_existing_files() {
        let root = std::env::temp_dir().join(format!(
            "loom-pi-fff-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = SettingsPaths {
            herdr_config: root.join("herdr.toml"),
            zed_settings: root.join("zed-settings.json"),
            zed_keymap: root.join("zed-keymap.json"),
            pi_fff_config: root.join("agent/pi-fff.json"),
            pi_adhd_flag: root.join("agent/.i-have-adhd-always"),
        };
        let fff = curated_settings()
            .into_iter()
            .find(|setting| setting.id == "pi:fff-override")
            .unwrap();
        assert!(apply_setting(&fff, &paths).unwrap());
        let created: Json =
            serde_json::from_str(&fs::read_to_string(&paths.pi_fff_config).unwrap()).unwrap();
        assert_eq!(created["mode"], "override");
        fs::write(&paths.pi_fff_config, "custom FFF config\n").unwrap();
        assert!(!apply_setting(&fff, &paths).unwrap());
        assert_eq!(
            fs::read_to_string(&paths.pi_fff_config).unwrap(),
            "custom FFF config\n"
        );
        let adhd = pi_adhd_setting();
        assert!(apply_setting(&adhd, &paths).unwrap());
        fs::write(&paths.pi_adhd_flag, "custom flag\n").unwrap();
        assert!(!apply_setting(&adhd, &paths).unwrap());
        assert_eq!(
            fs::read_to_string(&paths.pi_adhd_flag).unwrap(),
            "custom flag\n"
        );
        #[cfg(unix)]
        {
            // Existing defaults and an active flag have distinct dangling-link contracts.
            for path in [&paths.pi_fff_config, &paths.pi_adhd_flag] {
                fs::remove_file(path).unwrap();
                std::os::unix::fs::symlink(root.join("absent"), path).unwrap();
            }
            assert!(!apply_setting(&fff, &paths).unwrap());
            assert!(apply_setting(&adhd, &paths).is_err());
            assert!(!root.join("absent").exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
