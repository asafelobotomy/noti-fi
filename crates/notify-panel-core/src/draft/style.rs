//! Reading and writing the look-and-behaviour keys of a rule or zone (opacity, width, font, priority, ...).

use super::*;

pub(super) enum StyleKind {
    Num,
    Flag,
    Text,
}

pub(super) fn style_kind(key: &str) -> Option<StyleKind> {
    Some(match key {
        "opacity" | "width" | "fontSize" | "timeout" => StyleKind::Num,
        "clickThrough" | "noFocus" | "silent" | "media" | "stack" => StyleKind::Flag,
        "fontFamily" | "group" | "zone" | "priority" | "timerStyle" => StyleKind::Text,
        _ => return None,
    })
}

/// Set (or, for empty text, remove) one outcome key on a rule or zone object. Flags: "true"/"false" are both stored (an explicit false overrides
/// a later rule); `silent` false removes it.
pub(super) fn set_outcome(o: &mut Map<String, Value>, key: &str, text: &str) -> Result<(), String> {
    let kind = style_kind(key).ok_or_else(|| format!("unknown setting '{key}'"))?;
    let t = text.trim();
    let value = match kind {
        StyleKind::Text => (!t.is_empty()).then(|| json!(t)),
        StyleKind::Flag => match t {
            "" => None,
            "false" if key == "silent" => None,
            "true" => Some(json!(true)),
            "false" => Some(json!(false)),
            _ => return Err(format!("'{t}' is not true or false")),
        },
        StyleKind::Num => {
            if t.is_empty() {
                None
            } else {
                let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
                if !n.is_finite() || n < 0.0 {
                    return Err("must be zero or more".into());
                }
                Some(if n.fract() == 0.0 { json!(n as i64) } else { json!((n * 1000.0).round() / 1000.0) })
            }
        }
    };
    match value {
        Some(v) => o.insert(key.into(), v),
        None => o.remove(key),
    };
    Ok(())
}

/// The value of an outcome key as text ("" = not set).
pub(super) fn outcome_text(o: &Value, key: &str) -> String {
    match o.get(key) {
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::String(t)) => t.clone(),
        Some(v) => number_text(v),
        None => String::new(),
    }
}
