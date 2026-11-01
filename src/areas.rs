use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    declaration::{Area, Declaration},
    state::State,
    websocket::Ws,
};

#[derive(Deserialize)]
struct ExistingArea {
    area_id: String,
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    icon: Option<String>,
}

pub fn reconcile(
    ws: &mut Ws,
    declaration: &Declaration,
    state: &mut State,
    state_path: &Path,
) -> Result<()> {
    let mut existing = list(ws)?;

    for key in state.areas.keys().cloned().collect::<Vec<_>>() {
        if declaration.areas.contains_key(&key) {
            continue;
        }
        let area_id = state.areas[&key].clone();
        if existing.iter().any(|area| area.area_id == area_id) {
            log::info!("removing area {key}");
            ws.call(
                "config/area_registry/delete",
                object(json!({ "area_id": area_id })),
            )?;
            existing.retain(|area| area.area_id != area_id);
        }
        state.areas.remove(&key);
        state.save(state_path)?;
    }

    for (key, area) in &declaration.areas {
        let known = state.areas.get(key);
        let current = existing
            .iter()
            .find(|candidate| Some(&candidate.area_id) == known)
            .or_else(|| {
                existing
                    .iter()
                    .find(|candidate| same_name(&candidate.name, &area.name))
            });
        let area_id = match current {
            Some(found) => {
                if differs(found, area) {
                    log::info!("updating area {key}");
                    ws.call(
                        "config/area_registry/update",
                        object(json!({
                            "area_id": found.area_id,
                            "name": area.name,
                            "aliases": area.aliases,
                            "icon": area.icon,
                        })),
                    )?;
                }
                found.area_id.clone()
            }
            None => {
                log::info!("creating area {key}");
                let mut payload = object(json!({ "name": area.name, "aliases": area.aliases }));
                if let Some(icon) = &area.icon {
                    payload.insert("icon".to_string(), json!(icon));
                }
                let created = ws.call("config/area_registry/create", payload)?;
                created["area_id"]
                    .as_str()
                    .context("area creation returned no area_id")?
                    .to_string()
            }
        };
        state.areas.insert(key.clone(), area_id);
        state.save(state_path)?;
    }
    Ok(())
}

fn same_name(left: &str, right: &str) -> bool {
    normalized(left) == normalized(right)
}

fn normalized(name: &str) -> String {
    name.split_whitespace().collect::<String>().to_lowercase()
}

fn differs(found: &ExistingArea, area: &Area) -> bool {
    found.name != area.name
        || found.icon != area.icon
        || found.aliases.iter().collect::<BTreeSet<_>>()
            != area.aliases.iter().collect::<BTreeSet<_>>()
}

fn list(ws: &mut Ws) -> Result<Vec<ExistingArea>> {
    serde_json::from_value(ws.call("config/area_registry/list", Map::new())?)
        .context("parsing area list")
}

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{ExistingArea, differs, same_name};
    use crate::declaration::Area;

    #[test]
    fn alias_order_does_not_matter() {
        assert!(!differs(
            &ExistingArea {
                area_id: "kitchen".to_string(),
                name: "Kitchen".to_string(),
                aliases: vec!["cooking".to_string(), "food".to_string()],
                icon: None,
            },
            &Area {
                name: "Kitchen".to_string(),
                aliases: vec!["food".to_string(), "cooking".to_string()],
                icon: None,
            }
        ));
    }

    #[test]
    fn names_match_like_home_assistant_compares_them() {
        assert!(same_name("Living Room", "living room"));
        assert!(same_name("Living Room", "livingroom"));
        assert!(!same_name("Kitchen", "Kitchenette"));
    }
}
