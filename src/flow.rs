use anyhow::{Result, bail};
use serde_json::Value;

use crate::{api::Api, declaration, form};

pub fn drive(api: &Api, path: &str, start: Value, steps: &[Value], label: &str) -> Result<Value> {
    let mut response = api.post(path, &start)?;
    let mut remaining = steps.iter();
    let mut flow_id = String::new();
    loop {
        if response.status >= 400 {
            abort(api, path, &flow_id);
            bail!("{label}: HTTP {}: {}", response.status, response.body);
        }
        let result = response.body;
        log::debug!("{label}: flow result {result}");
        if let Some(id) = result["flow_id"].as_str() {
            flow_id = id.to_string();
        }
        match result["type"].as_str() {
            Some("form") | Some("menu") => {
                if let Some(errors) = result["errors"]
                    .as_object()
                    .filter(|errors| !errors.is_empty())
                {
                    abort(api, path, &flow_id);
                    bail!(
                        "{label}: step {} rejected: {}",
                        step_name(&result),
                        Value::Object(errors.clone())
                    );
                }
                let Some(input) = remaining.next() else {
                    abort(api, path, &flow_id);
                    bail!(
                        "{label}: Home Assistant asks for step {} but no further step is declared{}",
                        step_name(&result),
                        fields(&result)
                    );
                };
                let (input, missing) =
                    form::complete(&result["data_schema"], declaration::resolve_files(input)?);
                if !missing.is_empty() {
                    abort(api, path, &flow_id);
                    bail!(
                        "{label}: step {} is missing required fields: {}",
                        step_name(&result),
                        missing.join(", ")
                    );
                }
                response = api.post(&format!("{path}/{flow_id}"), &input)?;
            }
            Some("create_entry") => {
                if remaining.next().is_some() {
                    bail!("{label}: more steps declared than the flow asked for");
                }
                return Ok(result["result"].clone());
            }
            Some("abort") => bail!("{label}: aborted by Home Assistant: {}", result["reason"]),
            Some(other) => {
                abort(api, path, &flow_id);
                bail!("{label}: unsupported flow step type {other}");
            }
            None => bail!("{label}: unexpected response: {result}"),
        }
    }
}

fn step_name(result: &Value) -> &str {
    result["step_id"].as_str().unwrap_or("?")
}

fn fields(result: &Value) -> String {
    let names: Vec<&str> = result["data_schema"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|field| field["name"].as_str())
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(" (fields: {})", names.join(", "))
    }
}

fn abort(api: &Api, path: &str, flow_id: &str) {
    if !flow_id.is_empty() {
        let _ = api.delete(&format!("{path}/{flow_id}"));
    }
}
