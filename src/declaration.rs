use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Declaration {
    pub owner: Owner,
    #[serde(default)]
    pub areas: BTreeMap<String, Area>,
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

fn default_language() -> String {
    "en".to_string()
}

pub fn load(path: &Path) -> Result<Declaration> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading declaration {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing declaration {}", path.display()))
}
