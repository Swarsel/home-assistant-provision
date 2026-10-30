use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Declaration {
    pub owner: Owner,
}

#[derive(Deserialize)]
pub struct Owner {
    pub username: String,
}

pub fn load(path: &Path) -> Result<Declaration> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading declaration {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing declaration {}", path.display()))
}
