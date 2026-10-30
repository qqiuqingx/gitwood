//! Bundled theme parsing, safe CSS values, inheritance and per-variable fallback.
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

const LIGHT: &str = include_str!("builtin/light.json");
const DARK: &str = include_str!("builtin/dark.json");
const WALNUT: &str = include_str!("builtin/walnut.json");

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ThemeError {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub schema_version: u32,
    pub author: String,
    pub name: String,
    pub dark: bool,
    pub variables: BTreeMap<String, String>,
    pub errors: Vec<ThemeError>,
}

fn error(errors: &mut Vec<ThemeError>, code: &str, path: &str, message: &str) {
    errors.push(ThemeError {
        code: code.into(),
        path: path.into(),
        message: message.into(),
    });
}

fn source(name: &str) -> Option<&'static str> {
    match name {
        "light" => Some(LIGHT),
        "dark" => Some(DARK),
        "walnut" => Some(WALNUT),
        _ => None,
    }
}

/// Retrieve a built-in theme; unknown names yield Light and a structured error.
pub fn get_theme(name: &str) -> Theme {
    match source(name) {
        Some(json) => parse_document(json, name),
        None => {
            let mut theme = parse_theme(LIGHT);
            error(
                &mut theme.errors,
                "unknown_theme",
                "name",
                "Unknown built-in theme; using Light",
            );
            theme
        }
    }
}

/// Parse a document against the built-in registry. No filesystem or remote loads.
pub fn parse_theme(json: &str) -> Theme {
    parse_document(json, "<document>")
}

fn parse_document(json: &str, source_id: &str) -> Theme {
    let catalog: Value = serde_json::from_str(LIGHT).expect("bundled Light must be valid JSON");
    let defaults = catalog["colors"]
        .as_object()
        .expect("bundled variable catalog");
    let mut errors = Vec::new();
    let document: Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(err) => {
            error(&mut errors, "invalid_json", "$", &err.to_string());
            Value::Null
        }
    };
    let resolved = resolve(&document, source_id, &mut Vec::new(), &mut errors);
    let mut variables = BTreeMap::new();
    for (key, value) in defaults {
        let value = resolved["colors"].get(key).unwrap_or(value);
        let css = css_name(key);
        if let Some(lanes) = value.as_array() {
            variables.insert(format!("{css}-count"), lanes.len().to_string());
            for (i, lane) in lanes.iter().enumerate() {
                if let Some(color) = lane.as_str() {
                    variables.insert(format!("{css}-{i}"), color.into());
                }
            }
        } else if let Some(value) = value.as_str() {
            variables.insert(css, value.into());
        }
    }
    Theme {
        schema_version: 1,
        author: resolved["author"]
            .as_str()
            .unwrap_or("Gitwood contributors")
            .into(),
        name: resolved["name"].as_str().unwrap_or("Light").into(),
        dark: resolved["dark"].as_bool().unwrap_or(false),
        variables,
        errors,
    }
}

fn resolve(
    document: &Value,
    source_id: &str,
    stack: &mut Vec<String>,
    errors: &mut Vec<ThemeError>,
) -> Value {
    let dark = document["dark"].as_bool().unwrap_or(false);
    let mut base: Value =
        serde_json::from_str(if dark { DARK } else { LIGHT }).expect("bundled theme JSON");
    let Some(object) = document.as_object() else {
        error(
            errors,
            "invalid_document",
            "$",
            "Theme must be an object; using Light",
        );
        return base;
    };
    if document["schemaVersion"].as_u64() != Some(1) {
        error(
            errors,
            "unsupported_version",
            "schemaVersion",
            "Only schemaVersion 1 is supported; using default theme",
        );
        return base;
    }
    for key in object.keys() {
        if ![
            "$schema",
            "schemaVersion",
            "name",
            "author",
            "dark",
            "extends",
            "colors",
        ]
        .contains(&key.as_str())
        {
            error(errors, "unknown_field", key, "Unknown theme field");
        }
    }
    for key in ["name", "author"] {
        if !document[key].as_str().is_some_and(|s| !s.trim().is_empty()) {
            error(errors, "invalid_field", key, "Expected a nonempty string");
        }
    }
    if !document["dark"].is_boolean() {
        error(errors, "invalid_field", "dark", "Expected a boolean");
    }
    let id = source_id.to_owned();
    if stack.contains(&id) {
        error(
            errors,
            "inheritance_cycle",
            "extends",
            "Circular inheritance; using default theme",
        );
        return base;
    }
    stack.push(id);
    if let Some(parent) = object.get("extends") {
        if let Some(name) = parent.as_str() {
            if stack.iter().any(|id| id == name) {
                error(
                    errors,
                    "inheritance_cycle",
                    "extends",
                    "Circular inheritance; using default parent",
                );
            } else if let Some(json) = source(name) {
                let parent: Value = serde_json::from_str(json).expect("bundled parent JSON");
                base = resolve(&parent, name, stack, errors);
            } else {
                error(
                    errors,
                    "unknown_parent",
                    "extends",
                    "Parent must be light, dark or walnut",
                );
            }
        } else {
            error(
                errors,
                "invalid_field",
                "extends",
                "Expected a built-in theme name",
            );
        }
    }
    stack.pop();
    let catalog: Value = serde_json::from_str(LIGHT).expect("bundled variable catalog");
    if let Some(colors) = document["colors"].as_object() {
        for (key, value) in colors {
            let path = format!("colors.{key}");
            if catalog["colors"].get(key).is_none() {
                error(errors, "unknown_variable", &path, "Unknown theme variable");
            } else if !valid_value(key, value) {
                error(
                    errors,
                    "invalid_value",
                    &path,
                    "Invalid value; keeping inherited default",
                );
            } else {
                base["colors"][key] = value.clone();
            }
        }
    } else {
        error(
            errors,
            "invalid_field",
            "colors",
            "Expected a variable object",
        );
    }
    for key in ["name", "author"] {
        if document[key].as_str().is_some_and(|s| !s.trim().is_empty()) {
            base[key] = document[key].clone();
        }
    }
    if document["dark"].is_boolean() {
        base["dark"] = document["dark"].clone();
    }
    base
}

fn valid_color(value: &str) -> bool {
    value.strip_prefix('#').is_some_and(|s| {
        matches!(s.len(), 3 | 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

fn valid_value(key: &str, value: &Value) -> bool {
    if key == "graph.lanes" {
        return value.as_array().is_some_and(|a| {
            (1..=32).contains(&a.len()) && a.iter().all(|v| v.as_str().is_some_and(valid_color))
        });
    }
    let Some(s) = value.as_str() else {
        return false;
    };
    if key.ends_with(".family") {
        return !s.trim().is_empty()
            && s.len() <= 256
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" ,\"'._-".contains(&b));
    }
    if key == "font.weight.heading" {
        return [
            "100", "200", "300", "400", "500", "600", "700", "800", "900",
        ]
        .contains(&s);
    }
    if key == "font.editor.lineHeight" || key == "line.height.ui" {
        return decimal(s).is_some_and(|v| (1.0..=2.0).contains(&v));
    }
    if key.ends_with(".opacity") {
        return decimal(s).is_some_and(|v| (0.0..=1.0).contains(&v));
    }
    if [
        "list.row.height",
        "popup.height",
        "popup.padding",
        "popup.viewport.margin",
        "popup.width",
        "search.width",
        "tab.close.size",
        "toolbar.button.size",
        "tooltip.offset",
        "tree.disclosure.size",
        "tree.indent",
    ]
    .contains(&key)
    {
        return s
            .strip_suffix("px")
            .and_then(decimal)
            .is_some_and(|v| (0.0..=2048.0).contains(&v));
    }
    if key.starts_with("font.")
        || key.starts_with("space.")
        || ["splitter.size", "border.width", "focus.width"].contains(&key)
    {
        return s
            .strip_suffix("px")
            .and_then(decimal)
            .is_some_and(|v| (0.0..100.0).contains(&v));
    }
    valid_color(s)
}

fn decimal(s: &str) -> Option<f64> {
    let mut parts = s.split('.');
    let whole = parts.next()?;
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || (whole.len() > 1 && whole.starts_with('0'))
    {
        return None;
    }
    if let Some(fraction) = parts.next() {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    s.parse().ok()
}

fn css_name(key: &str) -> String {
    let mut result = String::from("--");
    for ch in key.chars() {
        if ch == '.' {
            result.push('-');
        } else if ch.is_ascii_uppercase() {
            result.push('-');
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn custom(overrides: Value) -> String {
        json!({"schemaVersion":1,"name":"Custom","author":"Author","dark":true,"extends":"walnut","colors":overrides}).to_string()
    }
    #[test]
    fn all_builtins_are_complete_and_valid() {
        for name in ["light", "dark", "walnut"] {
            let theme = get_theme(name);
            assert!(theme.errors.is_empty(), "{name}: {:?}", theme.errors);
            assert_eq!(theme.variables.len(), get_theme("light").variables.len());
            assert!(theme.variables.contains_key("--font-editor-line-height"));
            let value: Value = serde_json::from_str(source(name).unwrap()).unwrap();
            let default: Value = serde_json::from_str(LIGHT).unwrap();
            assert_eq!(
                value["colors"].as_object().unwrap().len(),
                default["colors"].as_object().unwrap().len()
            );
        }
    }
    #[test]
    fn inherits_only_overrides_and_flattens_arrays() {
        let theme = parse_theme(&custom(
            json!({"accent":"#1234", "graph.lanes":["#abc","#12345678"]}),
        ));
        assert!(theme.errors.is_empty());
        assert_eq!(theme.variables["--accent"], "#1234");
        assert_eq!(theme.variables["--background-primary"], "#32302f");
        assert_eq!(theme.variables["--graph-lanes-count"], "2");
        assert_eq!(theme.variables["--graph-lanes-1"], "#12345678");
    }
    #[test]
    fn reports_all_bad_variables_and_keeps_parent_values() {
        let theme = parse_theme(&custom(
            json!({"unknown":"#fff", "accent":"url(https://bad)", "graph.lanes":[], "font.ui.family":"url(evil)", "font.ui.size":"NaNpx", "syntax.number":3}),
        ));
        assert_eq!(theme.errors.len(), 6);
        assert_eq!(theme.variables, get_theme("walnut").variables);
    }
    #[test]
    fn malformed_version_missing_fields_and_unknown_parent_fall_back() {
        assert!(!parse_theme("{").errors.is_empty());
        let theme = parse_theme(r##"{"schemaVersion":2,"colors":{"accent":"#abc"}}"##);
        assert_eq!(theme.variables, get_theme("light").variables);
        assert_eq!(theme.errors[0].code, "unsupported_version");
        let missing = parse_theme(r#"{"schemaVersion":1,"extends":"absent"}"#);
        assert_eq!(missing.errors.len(), 5);
        assert_eq!(missing.variables, get_theme("light").variables);
        assert_eq!(get_theme("unknown").errors[0].code, "unknown_theme");
    }
    #[test]
    fn cyclic_inheritance_never_recurses_forever() {
        // A malformed bundled parent chain is rejected by the same resolver used by get_theme.
        let mut document: Value = serde_json::from_str(WALNUT).unwrap();
        document["extends"] = json!("walnut");
        let mut errors = Vec::new();
        let fallback = resolve(&document, "walnut", &mut Vec::new(), &mut errors);
        assert_eq!(errors[0].code, "inheritance_cycle");
        assert_eq!(fallback["colors"], document["colors"]);
        errors.clear();
        resolve(&document, "walnut", &mut vec!["walnut".into()], &mut errors);
        assert_eq!(errors[0].code, "inheritance_cycle");
        // Display names are not source IDs; a user may reuse a parent's display name.
        let theme = parse_theme(
            r#"{"schemaVersion":1,"name":"Walnut","author":"A","dark":true,"extends":"walnut","colors":{}}"#,
        );
        assert!(theme.errors.is_empty());
        assert_eq!(theme.variables, get_theme("walnut").variables);
    }
    #[test]
    fn rejects_css_injection_and_invalid_colors() {
        for bad in [
            "red",
            "#12",
            "#ggg",
            "#12345",
            "#ffffff; color:red",
            "var(--unsafe)",
            "#１２３",
        ] {
            assert!(!valid_color(bad));
        }
        for good in ["#abc", "#AbC0", "#abcdef", "#ABCDEF12"] {
            assert!(valid_color(good));
        }
    }
}
