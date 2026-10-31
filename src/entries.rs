use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    api::Api,
    declaration::{Declaration, Entry},
    flow,
    state::{EntryState, State},
    websocket::Ws,
};

const FLOW: &str = "/api/config/config_entries/flow";
const OPTIONS_FLOW: &str = "/api/config/config_entries/options/flow";
const ENTRIES: &str = "/api/config/config_entries/entry";

#[derive(Deserialize)]
struct ExistingEntry {
    entry_id: String,
    title: String,
}

pub fn reconcile(
    api: &Api,
    ws: &mut Ws,
    declaration: &Declaration,
    state: &mut State,
    state_path: &Path,
) -> Result<()> {
    remove_undeclared(api, declaration, state, state_path)?;
    for (name, entry) in &declaration.entries {
        let entry_id = ensure(api, name, entry, state, state_path)?;
        if let Some(title) = &entry.title {
            set_title(api, ws, &entry_id, title)?;
        }
        if !entry.options.is_empty() {
            log::info!("applying options of entry {name}");
            flow::drive(
                api,
                OPTIONS_FLOW,
                json!({ "handler": entry_id }),
                &entry.options,
                &format!("options of entry {name}"),
            )?;
        }
    }
    Ok(())
}

fn remove_undeclared(
    api: &Api,
    declaration: &Declaration,
    state: &mut State,
    state_path: &Path,
) -> Result<()> {
    let existing = list(api)?;
    for name in state.entries.keys().cloned().collect::<Vec<_>>() {
        if declaration.entries.contains_key(&name) {
            continue;
        }
        let entry_id = state.entries[&name].entry_id.clone();
        if existing.iter().any(|entry| entry.entry_id == entry_id) {
            log::info!("removing entry {name}");
            delete(api, &entry_id)?;
        }
        state.entries.remove(&name);
        state.save(state_path)?;
    }
    Ok(())
}

fn ensure(
    api: &Api,
    name: &str,
    entry: &Entry,
    state: &mut State,
    state_path: &Path,
) -> Result<String> {
    let existing = list(api)?;
    if let Some(known) = state.entries.get(name)
        && existing
            .iter()
            .any(|candidate| candidate.entry_id == known.entry_id)
    {
        if known.steps == entry.steps {
            return Ok(known.entry_id.clone());
        }
        log::info!("recreating entry {name} because its steps changed");
        delete(api, &known.entry_id)?;
    }
    log::info!("creating entry {name} ({})", entry.domain);
    let created = flow::drive(
        api,
        FLOW,
        json!({ "handler": entry.domain }),
        &entry.steps,
        &format!("entry {name}"),
    )?;
    let entry_id = created["entry_id"]
        .as_str()
        .context("config flow result without entry_id")?
        .to_string();
    remember(state, state_path, name, entry, entry_id.clone())?;
    Ok(entry_id)
}

fn remember(
    state: &mut State,
    state_path: &Path,
    name: &str,
    entry: &Entry,
    entry_id: String,
) -> Result<()> {
    state.entries.insert(
        name.to_string(),
        EntryState {
            entry_id,
            steps: entry.steps.clone(),
        },
    );
    state.save(state_path)
}

fn set_title(api: &Api, ws: &mut Ws, entry_id: &str, title: &str) -> Result<()> {
    let existing = list(api)?;
    let Some(current) = existing.iter().find(|entry| entry.entry_id == entry_id) else {
        return Ok(());
    };
    if current.title != title {
        log::info!("renaming entry {entry_id} to {title}");
        ws.call(
            "config_entries/update",
            object(json!({ "entry_id": entry_id, "title": title })),
        )?;
    }
    Ok(())
}

fn list(api: &Api) -> Result<Vec<ExistingEntry>> {
    let entries = api.get(ENTRIES)?.expect_success("listing entries")?;
    serde_json::from_value(entries).context("parsing entry list")
}

fn delete(api: &Api, entry_id: &str) -> Result<()> {
    api.delete(&format!("{ENTRIES}/{entry_id}"))?
        .expect_success("deleting entry")?;
    Ok(())
}

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}
