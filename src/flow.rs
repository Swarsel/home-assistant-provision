use anyhow::{Result, bail};
use serde_json::{Map, Value};

use crate::{api::Api, declaration};

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
                let input = with_form_defaults(&result, declaration::resolve_files(input)?);
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

fn with_form_defaults(result: &Value, input: Value) -> Value {
    let Value::Object(map) = input else {
        return input;
    };
    Value::Object(fill_defaults(result["data_schema"].as_array(), map))
}

fn fill_defaults(fields: Option<&Vec<Value>>, mut map: Map<String, Value>) -> Map<String, Value> {
    for field in fields.into_iter().flatten() {
        let Some(name) = field["name"].as_str() else {
            continue;
        };
        if field["type"] == "expandable" {
            let inner = match map.remove(name) {
                Some(Value::Object(inner)) => inner,
                _ => Map::new(),
            };
            map.insert(
                name.to_string(),
                Value::Object(fill_defaults(field["schema"].as_array(), inner)),
            );
            continue;
        }
        if map.contains_key(name) {
            continue;
        }
        if let Some(value) = prefill(field) {
            map.insert(name.to_string(), value);
        }
    }
    map
}

fn prefill(field: &Value) -> Option<Value> {
    if let Some(value) = field["description"]
        .get("suggested_value")
        .filter(|value| !value.is_null())
    {
        return Some(value.clone());
    }
    if let Some(value) = field.get("default") {
        return Some(value.clone());
    }
    if field["required"] != true {
        return None;
    }
    let selector = &field["selector"];
    if selector.get("boolean").is_some() {
        return Some(Value::Bool(false));
    }
    if let Some(options) = selector["select"]["options"].as_array() {
        return options.first().map(|option| {
            option
                .get("value")
                .cloned()
                .unwrap_or_else(|| option.clone())
        });
    }
    if let Some(number) = selector.get("number") {
        return Some(number.get("min").cloned().unwrap_or(Value::from(0)));
    }
    if selector.get("text").is_some() {
        return Some(Value::String(String::new()));
    }
    match field["type"].as_str() {
        Some("boolean") => Some(Value::Bool(false)),
        Some("string") => Some(Value::String(String::new())),
        Some("integer") | Some("float") => Some(Value::from(0)),
        Some("select") => field["options"].as_array()?.first().map(|option| {
            option
                .as_array()
                .and_then(|pair| pair.first())
                .unwrap_or(option)
                .clone()
        }),
        _ => None,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::with_form_defaults;

    #[test]
    fn fills_missing_fields_like_the_frontend_would() {
        let form = json!({
            "data_schema": [
                { "name": "broker", "required": true, "selector": { "text": { "type": "text" } } },
                { "name": "port", "required": true, "default": 1883, "selector": { "number": { "min": 1.0 } } },
                { "name": "username", "required": false, "selector": { "text": { "type": "text" } } },
                { "name": "note", "required": false, "description": { "suggested_value": "hi" } },
                { "name": "blank", "required": true, "default": "x", "description": { "suggested_value": null } },
                {
                    "name": "other_settings",
                    "type": "expandable",
                    "required": true,
                    "schema": [
                        { "name": "set_client_cert", "required": true, "selector": { "boolean": {} } },
                        { "name": "set_ca_cert", "required": true, "selector": { "select": { "options": ["off", "auto"] } } },
                        { "name": "transport", "required": true, "selector": { "select": { "options": [ { "label": "TCP", "value": "tcp" } ] } } },
                        { "name": "keepalive", "required": true, "selector": { "number": { "min": 15.0 } } },
                        { "name": "client_id", "required": false, "selector": { "text": { "type": "text" } } },
                    ]
                },
                { "name": "legacy", "required": true, "type": "select", "options": [["a", "A"], ["b", "B"]] },
            ]
        });
        let input = with_form_defaults(
            &form,
            json!({ "broker": "x", "other_settings": { "keepalive": 30 } }),
        );
        assert_eq!(
            input,
            json!({
                "broker": "x",
                "port": 1883,
                "note": "hi",
                "blank": "x",
                "other_settings": { "set_client_cert": false, "set_ca_cert": "off", "transport": "tcp", "keepalive": 30 },
                "legacy": "a",
            })
        );
    }
}
