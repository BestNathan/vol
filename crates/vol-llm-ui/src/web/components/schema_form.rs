use dioxus::prelude::*;

/// Renders a form from a JSON Schema.
///
/// `schema` — the JSON Schema object (with `properties` and optional `required`).
/// `value` — shared signal holding the current form data as `serde_json::Value`.
#[component]
pub fn SchemaForm(schema: serde_json::Value, value: Signal<serde_json::Value>) -> Element {
    let properties = schema.get("properties").and_then(|v| v.as_object());
    let required: std::collections::HashSet<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let Some(props) = properties else {
        return rsx! {
            div { class: "text-[#888] text-[12px]", "No parameters required" }
        };
    };

    rsx! {
        div { class: "space-y-2",
            {props.iter().map(|(key, prop_schema)| {
                let is_required = required.contains(key);
                let field_key = key.clone();
                rsx! { SchemaField {
                    field_key: field_key.clone(),
                    prop_schema: prop_schema.clone(),
                    value,
                    required: is_required,
                } }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

/// Renders a single form field based on its JSON Schema property definition.
#[component]
fn SchemaField(
    field_key: String,
    prop_schema: serde_json::Value,
    value: Signal<serde_json::Value>,
    required: bool,
) -> Element {
    let type_str = prop_schema
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("string");
    let label = prop_schema
        .get("title")
        .and_then(|t| t.as_str())
        .map(String::from)
        .unwrap_or_else(|| field_key.clone());
    let desc = prop_schema.get("description").and_then(|t| t.as_str());

    match type_str {
        "string" => render_string_field(
            field_key.clone(),
            &label,
            desc,
            &prop_schema,
            value,
            required,
        ),
        "number" | "integer" => {
            render_number_field(field_key.clone(), &label, desc, type_str, value, required)
        }
        "boolean" => render_boolean_field(field_key.clone(), &label, desc, value, required),
        "object" => render_object_field(field_key, &label, desc, &prop_schema, value, required),
        _ => rsx! {
            div { class: "text-[#888] text-[12px]", "Unsupported type: {type_str}" }
        },
    }
}

fn render_string_field(
    field_key: String,
    label: &str,
    desc: Option<&str>,
    prop_schema: &serde_json::Value,
    value: Signal<serde_json::Value>,
    required: bool,
) -> Element {
    if let Some(enum_vals) = prop_schema.get("enum").and_then(|v| v.as_array()) {
        let options: Vec<String> = enum_vals
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let sel_val = value.read()[&field_key].as_str().unwrap_or("").to_string();
        rsx! {
            div { class: "flex flex-col gap-1",
                label { class: "text-[12px] text-[#aaa] font-semibold",
                    "{label}"
                    if required { span { class: "text-[#c04040]", " *" } }
                }
                select {
                    class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                    value: "{sel_val}",
                    onchange: move |ev| {
                        value.write_unchecked()[field_key.clone()] = serde_json::Value::String(ev.value());
                    },
                    {options.iter().map(|opt| {
                        rsx! { option { value: "{opt}", "{opt}" } }
                    }).collect::<Vec<Element>>().into_iter()}
                }
                if let Some(d) = desc {
                    div { class: "text-[10px] text-[#666]", "{d}" }
                }
            }
        }
    } else {
        let txt_val = value.read()[&field_key].as_str().unwrap_or("").to_string();
        rsx! {
            div { class: "flex flex-col gap-1",
                label { class: "text-[12px] text-[#aaa] font-semibold",
                    "{label}"
                    if required { span { class: "text-[#c04040]", " *" } }
                }
                input {
                    r#type: "text",
                    class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                    value: "{txt_val}",
                    oninput: move |ev| {
                        value.write_unchecked()[field_key.clone()] = serde_json::Value::String(ev.value());
                    },
                }
                if let Some(d) = desc {
                    div { class: "text-[10px] text-[#666]", "{d}" }
                }
            }
        }
    }
}

fn render_number_field(
    field_key: String,
    label: &str,
    desc: Option<&str>,
    type_str: &str,
    value: Signal<serde_json::Value>,
    required: bool,
) -> Element {
    let num_str = value.read()[&field_key]
        .as_number()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "0".to_string());
    let type_str_owned = type_str.to_string();
    rsx! {
        div { class: "flex flex-col gap-1",
            label { class: "text-[12px] text-[#aaa] font-semibold",
                "{label}"
                if required { span { class: "text-[#c04040]", " *" } }
            }
            input {
                r#type: "number",
                class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                value: "{num_str}",
                oninput: move |ev| {
                    let field_key = field_key.clone();
                    let v: serde_json::Value = if type_str_owned == "integer" {
                        serde_json::Value::Number(ev.value().parse::<i64>().unwrap_or(0).into())
                    } else {
                        serde_json::Value::Number(serde_json::Number::from_f64(ev.value().parse::<f64>().unwrap_or(0.0)).unwrap_or(serde_json::Number::from(0)))
                    };
                    value.write_unchecked()[field_key] = v;
                },
            }
            if let Some(d) = desc {
                div { class: "text-[10px] text-[#666]", "{d}" }
            }
        }
    }
}

fn render_boolean_field(
    field_key: String,
    label: &str,
    desc: Option<&str>,
    value: Signal<serde_json::Value>,
    required: bool,
) -> Element {
    rsx! {
        div { class: "flex items-center gap-2",
            input {
                r#type: "checkbox",
                checked: value.read()[&field_key].as_bool().unwrap_or(false),
                oninput: move |ev| {
                    value.write_unchecked()[field_key.clone()] = serde_json::Value::Bool(ev.checked());
                },
            }
            label { class: "text-[12px] text-[#aaa]",
                "{label}"
                if required { span { class: "text-[#c04040]", " *" } }
            }
            if let Some(d) = desc {
                div { class: "text-[10px] text-[#666]", "{d}" }
            }
        }
    }
}

fn render_object_field(
    _field_key: String,
    label: &str,
    desc: Option<&str>,
    prop_schema: &serde_json::Value,
    value: Signal<serde_json::Value>,
    required: bool,
) -> Element {
    rsx! {
        div { class: "border border-[#3a3a55] rounded p-2",
            div { class: "text-[12px] text-[#888] font-semibold mb-2",
                "{label}"
                if required { span { class: "text-[#c04040]", " *" } }
            }
            SchemaForm {
                schema: prop_schema.clone(),
                value,
            }
            if let Some(d) = desc {
                div { class: "text-[10px] text-[#666] mt-1", "{d}" }
            }
        }
    }
}
