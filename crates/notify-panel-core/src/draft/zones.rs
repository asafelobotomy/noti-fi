//! Editing zones in the draft.

use super::*;

impl Draft {
    /// Add a zone in the first free corner (or the top-right). Returns its id.
    pub fn add_zone(&mut self) -> String {
        let zones = self.zones_mut();
        let mut n = zones.len() + 1;
        while zones.contains_key(&format!("zone{n}")) {
            n += 1;
        }
        let id = format!("zone{n}");
        let used: Vec<&str> = zones.values().filter_map(|z| z.get("anchor").and_then(Value::as_str)).collect();
        let anchor = ["top-right", "top-left", "bottom-right", "bottom-left", "top-center", "bottom-center", "middle-right", "middle-left", "center"].into_iter().find(|a| !used.contains(a)).unwrap_or("top-right");
        zones.insert(id.clone(), json!({"anchor": anchor, "accent": "#34d399", "label": format!("Zone {n}")}));
        id
    }

    /// Remove a zone. Refused for the last zone and for the default zone; rules that sent popups there stop choosing a zone.
    pub fn delete_zone(&mut self, id: &str) -> Result<usize, String> {
        if id == "system" {
            return Err("System is the one group that always stays: it is how the system reaches you".into());
        }
        if self.zones_mut().len() <= 1 {
            return Err("there must be at least one zone".into());
        }
        if self.value.get("defaultZone").and_then(Value::as_str) == Some(id) {
            return Err("this is the default zone; choose another default first".into());
        }
        if self.zones_mut().remove(id).is_none() {
            return Err(format!("no zone '{id}'"));
        }
        let mut cleared = 0;
        for r in self.rules_mut().iter_mut().filter_map(Value::as_object_mut) {
            if r.get("zone").and_then(Value::as_str) == Some(id) {
                r.remove("zone");
                cleared += 1;
            }
        }
        Ok(cleared)
    }

    /// `field` is `label`, `anchor` or `accent`. An empty label or accent removes it.
    pub fn set_zone_field(&mut self, id: &str, field: &str, text: &str) -> Result<(), String> {
        if !["label", "anchor", "accent"].contains(&field) {
            return Err(format!("unknown zone field '{field}'"));
        }
        if field == "anchor" && !ANCHORS.contains(&text) {
            return Err(format!("'{text}' is not a place popups can go"));
        }
        let Some(z) = self.zones_mut().get_mut(id).and_then(Value::as_object_mut) else { return Err(format!("no zone '{id}'")) };
        if text.is_empty() && field != "anchor" {
            z.remove(field);
        } else {
            z.insert(field.into(), json!(text));
        }
        if field == "anchor" {
            if text == "free" {
                // a free zone needs a position; start in the middle of the screen
                z.entry("x").or_insert(json!(0.5));
                z.entry("y").or_insert(json!(0.5));
            } else {
                z.remove("x");
                z.remove("y");
            }
        }
        Ok(())
    }

    /// Put a zone at an exact place on the screen: `x` and `y` are 0..1 from the left and top edges. Makes the zone `free`.
    pub fn set_zone_position(&mut self, id: &str, x: f64, y: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("the position must be a number".into());
        }
        let r = |v: f64| (v.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
        let Some(z) = self.zones_mut().get_mut(id).and_then(Value::as_object_mut) else { return Err(format!("no zone '{id}'")) };
        z.insert("anchor".into(), json!("free"));
        z.insert("x".into(), json!(r(x)));
        z.insert("y".into(), json!(r(y)));
        Ok(())
    }

    /// The same keys on a zone (everything but `group`).
    pub fn set_zone_style(&mut self, id: &str, key: &str, text: &str) -> Result<(), String> {
        if !STYLE_KEYS.contains(&key) || key == "group" {
            return Err(format!("a zone has no setting '{key}'"));
        }
        let z = self.zones_mut().get_mut(id).and_then(Value::as_object_mut).ok_or_else(|| format!("no zone '{id}'"))?;
        set_outcome(z, key, text)
    }
}
