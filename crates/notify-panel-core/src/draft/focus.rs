//! Editing focus modes in the draft.

use super::*;

impl Draft {
    /// The focus modes as the UI shows them: name, label, minimum priority and the allow list as text (one entry per line, `field:pattern`
    /// pairs separated by ` and `; a bare pattern means the app name).
    pub fn focus_view(&self) -> Value {
        let fallback = Draft::default_focus();
        let focus = self.value.get("focus").unwrap_or(&fallback);
        let Some(modes) = focus.get("modes").and_then(Value::as_object) else { return json!([]) };
        Value::Array(
            modes
                .iter()
                .map(|(name, m)| {
                    let allow: Vec<String> = m
                        .get("allow")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_object)
                                .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|v| format!("{k}:{v}"))).collect::<Vec<_>>().join(" and "))
                                .collect()
                        })
                        .unwrap_or_default();
                    json!({
                        "name": name,
                        "label": m.get("label").and_then(Value::as_str).unwrap_or(""),
                        "minPriority": m.get("minPriority").and_then(Value::as_str).unwrap_or("urgent"),
                        "allow": allow.join("\n"),
                    })
                })
                .collect(),
        )
    }

    /// Add a mode that lets only urgent notifications through. Returns its name.
    pub fn add_focus_mode(&mut self) -> String {
        let modes = self.focus_modes_mut();
        let mut n = modes.len() + 1;
        while modes.contains_key(&format!("mode{n}")) {
            n += 1;
        }
        let name = format!("mode{n}");
        modes.insert(name.clone(), json!({"label": format!("Focus {n}"), "minPriority": "urgent"}));
        name
    }

    pub fn delete_focus_mode(&mut self, name: &str) -> Result<(), String> {
        if self.focus_modes_mut().remove(name).is_none() {
            return Err(format!("no focus mode '{name}'"));
        }
        Ok(())
    }

    /// `field` is `label`, `minPriority` (low, normal, high, urgent) or `allow` (see [`Draft::focus_view`]).
    pub fn set_focus_field(&mut self, name: &str, field: &str, text: &str) -> Result<(), String> {
        let allow = if field == "allow" { Some(parse_allow(text)?) } else { None };
        let m = self.focus_modes_mut().get_mut(name).and_then(Value::as_object_mut).ok_or_else(|| format!("no focus mode '{name}'"))?;
        match field {
            "label" => {
                if text.trim().is_empty() { m.remove("label") } else { m.insert("label".into(), json!(text.trim())) };
            }
            "minPriority" => {
                if !["low", "normal", "high", "urgent"].contains(&text) {
                    return Err(format!("'{text}' is not a priority"));
                }
                m.insert("minPriority".into(), json!(text));
            }
            "allow" => {
                let list = allow.unwrap_or_default();
                if list.is_empty() { m.remove("allow") } else { m.insert("allow".into(), Value::Array(list)) };
            }
            _ => return Err(format!("unknown focus setting '{field}'")),
        }
        Ok(())
    }
}

/// Parse the allow list text: one entry per line, `field:pattern` pairs joined with ` and `; a line without a field is an app-name pattern.
fn parse_allow(text: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut entry = Map::new();
        for part in line.split(" and ").map(str::trim).filter(|p| !p.is_empty()) {
            let (field, pat) = match part.split_once(':') {
                Some((f, p)) if MATCH_FIELDS.contains(&f.trim()) => (f.trim(), p.trim()),
                _ => ("appName", part),
            };
            if pat.is_empty() {
                return Err(format!("'{line}': a pattern is empty"));
            }
            entry.insert(field.into(), json!(pat));
        }
        out.push(Value::Object(entry));
    }
    Ok(out)
}
