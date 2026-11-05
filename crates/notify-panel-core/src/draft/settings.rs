//! The General settings (theme, history, sound ...) as a flat list of fields.

use super::*;

impl Draft {
    /// Everything the Zones and Appearance pages show, with the daemon's defaults filled in for missing keys.
    pub fn settings_view(&self) -> Value {
        let g = |path: &str| lookup(&self.value, path).cloned();
        let mut out = Map::new();
        for (path, default) in SETTING_DEFAULTS {
            out.insert((*path).into(), g(path).unwrap_or_else(|| default_value(default)));
        }
        let mut zones = Vec::new();
        if let Some(z) = self.value.get("zones").and_then(Value::as_object) {
            for (id, v) in z {
                let used = self.rules().iter().filter(|r| r.zone == *id).count();
                zones.push(json!({
                    "id": id,
                    "label": v.get("label").and_then(Value::as_str).unwrap_or(""),
                    "anchor": v.get("anchor").and_then(Value::as_str).unwrap_or("top-right"),
                    "accent": v.get("accent").and_then(Value::as_str).unwrap_or(""),
                    "style": STYLE_KEYS.iter().filter(|k| **k != "group").map(|k| (k.to_string(), json!(outcome_text(v, k)))).collect::<Map<String, Value>>(),
                    "x": v.get("x").and_then(Value::as_f64).unwrap_or(0.5),
                    "y": v.get("y").and_then(Value::as_f64).unwrap_or(0.5),
                    "rules": used,
                }));
            }
        }
        json!({"values": out, "zones": zones})
    }

    /// Set a scalar setting by dotted path (`theme.card`, `sound.enabled`, `maxVisible`...). Only known settings are accepted.
    /// Text for a text setting; "" removes the key (the daemon then uses its default). Numbers and booleans are parsed.
    pub fn set_setting(&mut self, path: &str, text: &str) -> Result<(), String> {
        let Some((_, kind)) = SETTING_DEFAULTS.iter().find(|(p, _)| *p == path) else { return Err(format!("unknown setting '{path}'")) };
        let value = match *kind {
            Kind::Text(_) | Kind::Color(_) => {
                if text.is_empty() { None } else { Some(json!(text)) }
            }
            Kind::Bool(_) => match text {
                "true" => Some(json!(true)),
                "false" => Some(json!(false)),
                _ => return Err(format!("'{text}' is not true or false")),
            },
            Kind::Num(_) => {
                let t = text.trim();
                if t.is_empty() {
                    None
                } else {
                    let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
                    if n < 0.0 || !n.is_finite() {
                        return Err("must be zero or more".into());
                    }
                    Some(if n.fract() == 0.0 { json!(n as i64) } else { json!(n) })
                }
            }
        };
        let parts: Vec<&str> = path.split('.').collect();
        let mut cur = self.value.as_object_mut().expect("draft is an object");
        for k in &parts[..parts.len() - 1] {
            let e = cur.entry(*k).or_insert_with(|| json!({}));
            if !e.is_object() {
                *e = json!({});
            }
            cur = e.as_object_mut().unwrap();
        }
        let last = parts[parts.len() - 1];
        match value {
            Some(v) => cur.insert(last.into(), v),
            None => cur.remove(last),
        };
        Ok(())
    }
}

fn default_value(k: &Kind) -> Value {
    match k {
        Kind::Text(d) | Kind::Color(d) => json!(d),
        Kind::Bool(b) => json!(b),
        Kind::Num(d) => json!(*d as i64),
    }
}
