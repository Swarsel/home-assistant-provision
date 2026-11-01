use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

pub fn complete(schema: &Value, input: Value) -> (Value, Vec<String>) {
    let Value::Object(declared) = input else {
        return (input, Vec::new());
    };
    let mut missing = Vec::new();
    let data = merge(schema_fields(schema), declared, "", &mut missing);
    (Value::Object(data), missing)
}

fn schema_fields(schema: &Value) -> &[Value] {
    schema.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn merge(
    fields: &[Value],
    mut declared: Map<String, Value>,
    prefix: &str,
    missing: &mut Vec<String>,
) -> Map<String, Value> {
    let mut data = Map::new();
    for field in fields {
        let Some(name) = field["name"].as_str() else {
            continue;
        };
        let path = format!("{prefix}{name}");
        let value = match declared.remove(name) {
            Some(Value::Object(inner)) if field["type"] == "expandable" => {
                Some(Value::Object(merge(
                    schema_fields(&field["schema"]),
                    inner,
                    &format!("{path}."),
                    missing,
                )))
            }
            Some(value) => Some(value),
            None => initial(field, &path, missing).map(whole_numbers_as_integers),
        };
        match value {
            Some(value) => {
                data.insert(name.to_string(), value);
            }
            None if field["required"] == true => missing.push(path),
            None => {}
        }
    }
    data.extend(declared);
    data
}

fn initial(field: &Value, path: &str, missing: &mut Vec<String>) -> Option<Value> {
    if let Some(suggested) = present(&field["description"]["suggested_value"]) {
        return Some(with_first_choice(field, suggested));
    }
    if let Some(default) = field.get("default") {
        return Some(with_first_choice(field, default.clone()));
    }
    if field["type"] == "expandable" {
        let mut section_missing = Vec::new();
        let section = merge(
            schema_fields(&field["schema"]),
            Map::new(),
            &format!("{path}."),
            &mut section_missing,
        );
        if field["required"] != true && section.is_empty() {
            return None;
        }
        missing.extend(section_missing);
        return Some(Value::Object(section));
    }
    if field["required"] != true {
        return None;
    }
    let value = match field["type"].as_str() {
        Some("boolean") => json!(false),
        Some("string") => json!(""),
        Some("integer") => field.get("valueMin").cloned().unwrap_or(json!(0)),
        Some("constant") => field.get("value")?.clone(),
        Some("float") => json!(0.0),
        Some("select") => {
            let first = field["options"].as_array()?.first()?;
            first
                .as_array()
                .and_then(|pair| pair.first())
                .unwrap_or(first)
                .clone()
        }
        Some("positive_time_period_dict") => json!({ "hours": 0, "minutes": 0, "seconds": 0 }),
        _ => selector_initial(field.get("selector")?)?,
    };
    Some(value)
}

fn with_first_choice(field: &Value, value: Value) -> Value {
    let Some(first) = field["selector"]["choose"]["choices"]
        .as_object()
        .and_then(|choices| choices.keys().next())
    else {
        return value;
    };
    let mut wrapped = Map::new();
    wrapped.insert("active_choice".to_string(), json!(first));
    wrapped.insert(first.clone(), value);
    Value::Object(wrapped)
}

fn selector_initial(selector: &Value) -> Option<Value> {
    let (kind, config) = selector.as_object()?.iter().next()?;
    let multiple = config["multiple"] == true;
    let single_or_list = |single: Value| if multiple { json!([]) } else { single };
    let value = match kind.as_str() {
        "action" | "condition" | "trigger" => json!([]),
        "addon" | "attribute" | "file" | "icon" | "serial_port" | "template" | "theme" => json!(""),
        "area" | "device" | "entity" | "label" | "object" | "state" | "text" => {
            single_or_list(json!(""))
        }
        "media" => single_or_list(json!({})),
        "device_class" | "state_class" if multiple => json!([]),
        "boolean" => json!(false),
        "choose" => choose_initial(config),
        "color_rgb" => json!([0, 0, 0]),
        "color_temp" => color_temp_initial(config),
        "constant" => config.get("value")?.clone(),
        "country" => config["countries"].get(0)?.clone(),
        "language" => config["languages"].get(0)?.clone(),
        "date" => json!(today()),
        "datetime" => json!(format!("{} 00:00:00", today())),
        "duration" => json!({ "hours": 0, "minutes": 0, "seconds": 0 }),
        "number" => present(&config["min"]).unwrap_or(json!(0)),
        "numeric_threshold" => numeric_threshold_initial(config),
        "select" => select_initial(config)?,
        "target" => json!({}),
        "time" => json!("00:00:00"),
        _ => return None,
    };
    Some(value)
}

fn choose_initial(config: &Value) -> Value {
    let Some((first, choice)) = config["choices"]
        .as_object()
        .and_then(|choices| choices.iter().next())
    else {
        return json!({});
    };
    let mut value = Map::new();
    value.insert("active_choice".to_string(), json!(first));
    if let Some(child) = selector_initial(&choice["selector"]) {
        value.insert(first.clone(), child);
    }
    Value::Object(value)
}

fn color_temp_initial(config: &Value) -> Value {
    if config["unit"] == "kelvin" {
        return present(&config["min"]).unwrap_or(json!(2700));
    }
    present(&config["min"])
        .or_else(|| present(&config["min_mireds"]))
        .unwrap_or(json!(153))
}

fn numeric_threshold_initial(config: &Value) -> Value {
    if config["mode"] == "changed" {
        return json!({ "type": "any" });
    }
    json!({
        "type": "above",
        "value": {
            "number": present(&config["number"]["min"]).unwrap_or(json!(0)),
            "active_choice": "number",
        },
    })
}

fn select_initial(config: &Value) -> Option<Value> {
    let first = config["options"].as_array()?.first()?;
    let value = if first.is_string() {
        first.clone()
    } else {
        first["value"].clone()
    };
    Some(if config["multiple"] == true {
        json!([value])
    } else {
        value
    })
}

fn present(value: &Value) -> Option<Value> {
    (!value.is_null()).then(|| value.clone())
}

fn whole_numbers_as_integers(value: Value) -> Value {
    match value {
        Value::Number(number) => match number.as_f64() {
            Some(float) if number.is_f64() && float.fract() == 0.0 && float.abs() < 1e15 => {
                Value::from(float as i64)
            }
            _ => Value::Number(number),
        },
        Value::Array(items) => {
            Value::Array(items.into_iter().map(whole_numbers_as_integers).collect())
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, inner)| (key, whole_numbers_as_integers(inner)))
                .collect(),
        ),
        other => other,
    }
}

fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 86_400);
    date_from_days(days)
}

fn date_from_days(mut days: u64) -> String {
    let mut year = 1970;
    while days >= days_in_year(year) {
        days -= days_in_year(year);
        year += 1;
    }
    let february = if days_in_year(year) == 366 { 29 } else { 28 };
    let mut month = 1;
    for length in [31, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31] {
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", days + 1)
}

fn days_in_year(year: u64) -> u64 {
    if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
        366
    } else {
        365
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{complete, date_from_days};

    #[test]
    fn fills_the_form_like_the_frontend() {
        let schema = json!([
            { "name": "broker", "required": true, "selector": { "text": {} } },
            { "name": "port", "required": true, "default": 1883, "type": "integer", "selector": { "number": { "min": 1.0 } } },
            { "name": "protocol", "required": true, "default": "5", "selector": { "select": { "options": ["5", "3.1.1"] } } },
            { "name": "username", "required": false, "selector": { "text": {} } },
            { "name": "note", "required": false, "default": "x", "description": { "suggested_value": "hi" } },
            { "name": "interval", "required": true, "selector": { "number": { "min": 15.0 } } },
            { "name": "mode", "required": true, "type": "select", "options": [["a", "A"], ["b", "B"]] },
            {
                "name": "other_settings",
                "type": "expandable",
                "required": true,
                "schema": [
                    { "name": "set_client_cert", "required": true, "selector": { "boolean": {} } },
                    { "name": "set_ca_cert", "required": true, "selector": { "select": { "options": ["off", "auto"] } } },
                    { "name": "transport", "required": true, "default": "tcp", "selector": { "select": { "options": [ { "label": "TCP", "value": "tcp" } ] } } },
                    { "name": "client_id", "required": false, "selector": { "text": {} } },
                ]
            },
            {
                "name": "advanced",
                "type": "expandable",
                "required": false,
                "schema": [ { "name": "debug", "required": false, "selector": { "boolean": {} } } ]
            },
        ]);
        let (data, missing) = complete(
            &schema,
            json!({ "broker": "127.0.0.1", "other_settings": { "set_ca_cert": "auto" } }),
        );
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(
            data,
            json!({
                "broker": "127.0.0.1",
                "port": 1883,
                "protocol": "5",
                "note": "hi",
                "interval": 15,
                "mode": "a",
                "other_settings": { "set_client_cert": false, "set_ca_cert": "auto", "transport": "tcp" },
            })
        );
    }

    #[test]
    fn reports_required_fields_the_frontend_cannot_fill() {
        let schema = json!([
            { "name": "location", "required": true, "selector": { "location": {} } },
            { "name": "choice", "required": true, "selector": { "select": { "options": [] } } },
            {
                "name": "section",
                "type": "expandable",
                "required": true,
                "schema": [ { "name": "timezone", "required": true, "selector": { "timezone": {} } } ]
            },
        ]);
        let (_, missing) = complete(&schema, json!({}));
        assert_eq!(missing, vec!["location", "choice", "section.timezone"]);
    }

    #[test]
    fn wraps_defaults_of_choose_selectors_in_their_first_choice() {
        let schema = json!([
            {
                "name": "target",
                "required": true,
                "default": 5,
                "selector": { "choose": { "choices": { "number": { "selector": { "number": {} } }, "entity": { "selector": { "entity": {} } } } } }
            },
            {
                "name": "fallback",
                "required": true,
                "selector": { "choose": { "choices": { "entity": { "selector": { "entity": {} } } } } }
            },
        ]);
        let (data, _) = complete(&schema, json!({}));
        assert_eq!(
            data["target"],
            json!({ "active_choice": "number", "number": 5 })
        );
        assert_eq!(
            data["fallback"],
            json!({ "active_choice": "entity", "entity": "" })
        );
    }

    #[test]
    fn keeps_declared_fields_unknown_to_the_schema() {
        let (data, _) = complete(&json!([]), json!({ "typo": 1 }));
        assert_eq!(data, json!({ "typo": 1 }));
    }

    #[test]
    fn converts_days_since_epoch_to_dates() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(11_017), "2000-03-01");
        assert_eq!(date_from_days(19_782), "2024-02-29");
    }
}
