use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
pub struct Declaration {
    pub owner: Owner,
    #[serde(default)]
    pub areas: BTreeMap<String, Area>,
    #[serde(default)]
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
pub struct Owner {
    pub name: String,
    pub username: String,
    #[serde(default = "default_language")]
    pub language: String,
}

#[derive(Deserialize)]
pub struct Area {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Deserialize)]
pub struct Entry {
    pub domain: String,
    #[serde(default)]
    pub steps: Vec<Value>,
    #[serde(default)]
    pub options: Vec<Value>,
    #[serde(default)]
    pub title: Option<String>,
}

fn default_language() -> String {
    "en".to_string()
}

pub fn load(path: &Path) -> Result<Declaration> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading declaration {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing declaration {}", path.display()))
}

pub fn resolve_files(value: &Value) -> Result<Value> {
    match value {
        Value::Object(map) => {
            if let (1, Some(Value::String(path))) = (map.len(), map.get("_file")) {
                let content = fs::read_to_string(path)
                    .with_context(|| format!("reading secret file {path}"))?;
                return Ok(Value::String(
                    content.trim_end_matches(['\n', '\r']).to_string(),
                ));
            }
            let mut resolved = Map::new();
            for (key, inner) in map {
                resolved.insert(key.clone(), resolve_files(inner)?);
            }
            Ok(Value::Object(resolved))
        }
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(resolve_files)
                .collect::<Result<Vec<_>>>()?,
        )),
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use serde_json::json;

    use super::resolve_files;

    #[test]
    fn replaces_file_markers_with_trimmed_contents() {
        let secret = env::temp_dir().join(format!("provision-secret-{}", std::process::id()));
        fs::write(&secret, "hunter2\n").unwrap();
        let input = json!({
            "broker": "127.0.0.1",
            "password": { "_file": secret.to_str().unwrap() },
            "nested": [ { "_file": secret.to_str().unwrap() }, { "_file": "x", "other": 1 } ],
        });
        let resolved = resolve_files(&input).unwrap();
        fs::remove_file(&secret).unwrap();
        assert_eq!(resolved["password"], "hunter2");
        assert_eq!(resolved["nested"][0], "hunter2");
        assert_eq!(resolved["nested"][1], json!({ "_file": "x", "other": 1 }));
    }
}
