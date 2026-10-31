use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub areas: BTreeMap<String, String>,
    #[serde(default)]
    pub entries: BTreeMap<String, EntryState>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EntryState {
    pub entry_id: String,
    pub steps: Vec<Value>,
}

impl State {
    pub fn load(path: &Path) -> Result<State> {
        if !path.exists() {
            return Ok(State::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading state {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing state {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("writing state {}", temporary.display()))?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("replacing state {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use serde_json::json;

    use super::{EntryState, State};

    #[test]
    fn round_trips_through_disk() {
        let path = env::temp_dir().join(format!("provision-state-{}.json", std::process::id()));
        let mut state = State::default();
        state
            .areas
            .insert("office".to_string(), "area123".to_string());
        state.entries.insert(
            "mqtt".to_string(),
            EntryState {
                entry_id: "entry456".to_string(),
                steps: vec![json!({ "broker": "127.0.0.1" })],
            },
        );
        state.save(&path).unwrap();
        let loaded = State::load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(loaded.areas["office"], "area123");
        assert_eq!(loaded.entries["mqtt"].entry_id, "entry456");
        assert_eq!(
            loaded.entries["mqtt"].steps,
            vec![json!({ "broker": "127.0.0.1" })]
        );
    }

    #[test]
    fn missing_state_starts_empty() {
        let loaded = State::load(&env::temp_dir().join("provision-state-missing.json")).unwrap();
        assert!(loaded.areas.is_empty() && loaded.entries.is_empty());
    }
}
