//! How the panel's data is turned into what the QML pages show (history, rules, apps), and how the editing draft follows the file.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use notify_panel_core::draft::Draft;
use notify_panel_core::view;
use notify_panel_core::worker::{Cmd, Snapshot};
use serde_json::{Value, json};

use super::qobject;

impl qobject::PanelModel {
    pub(crate) fn refresh_history_view(mut self: Pin<&mut Self>) {
        let (json, shown, total) = {
            let me = self.as_ref();
            let r = me.rust();
            let rows = view::filter_history(&r.history_all, &r.filter_query, r.filter_missed);
            (view::history_json(&rows), rows.len() as i32, r.history_all.len() as i32)
        };
        self.as_mut().set_history_json(QString::from(&json));
        self.as_mut().set_history_shown(shown);
        self.as_mut().set_history_total(total);
    }

    pub(crate) fn on_snapshot(mut self: Pin<&mut Self>, s: Snapshot) {
        self.as_mut().set_daemon_version(QString::from(&s.version));
        self.as_mut().set_dnd(s.status.dnd);
        self.as_mut().set_focus_mode(QString::from(&s.status.focus));
        let modes: Vec<Value> = s.config.focus.modes.iter().map(|(n, m)| json!({"name": n, "label": m.label.clone().unwrap_or_else(|| n.clone()), "minPriority": m.min_priority.clone().unwrap_or_else(|| "urgent".into())})).collect();
        self.as_mut().set_focus_modes_json(QString::from(&json!(modes).to_string()));
        self.as_mut().set_holding(QString::from(&s.status.holding));
        self.as_mut().set_hold_text(QString::from(view::hold_text(true, &s.status.holding)));
        self.as_mut().set_active(s.status.active as i32);
        self.as_mut().set_queued(s.status.queued as i32);
        self.as_mut().set_zones_json(QString::from(&view::zones_json(&s.status, &s.config)));
        self.as_mut().rust_mut().history_all = s.history;
        self.as_mut().refresh_history_view();
        self.as_mut().push_tray();
        self.sync_draft(&s.raw);
    }

    /// Keep the editor's draft in step with the file: follow hand edits while there are no unsaved changes,
    /// and flag them when there are.
    pub(crate) fn sync_draft(mut self: Pin<&mut Self>, raw: &str) {
        self.as_mut().rust_mut().latest_raw = raw.to_string();
        let (loaded, dirty, base_matches) = {
            let me = self.as_ref();
            let r = me.rust();
            (r.draft_loaded, r.draft.is_dirty(), r.draft.base_raw() == raw)
        };
        if !loaded || (!dirty && !base_matches) {
            if let Ok(d) = Draft::from_raw(raw) {
                let mut r = self.as_mut().rust_mut();
                r.draft = d;
                r.draft_loaded = true;
                self.as_mut().set_external_changed(false);
                self.after_edit();
                return;
            }
        }
        self.as_mut().set_external_changed(dirty && !base_matches);
    }

    /// Something about the draft changed: refresh what the UI shows, and ask the daemon to validate and simulate it.
    pub(crate) fn after_edit(mut self: Pin<&mut Self>) {
        let (vt, st, config, request) = {
            let mut r = self.as_mut().rust_mut();
            r.validate_token += 1;
            r.sim_token += 1;
            (r.validate_token, r.sim_token, r.draft.value().clone(), r.sim.to_request(r.draft.value()))
        };
        self.as_mut().set_checking(true);
        self.as_ref().send_cmd(Cmd::Validate { token: vt, config });
        self.as_ref().send_cmd(Cmd::Simulate { token: st, request });
        self.refresh_rules_view();
    }

    pub(crate) fn refresh_rules_view(mut self: Pin<&mut Self>) {
        let (json, count, zones, dirty) = {
            let me = self.as_ref();
            let r = me.rust();
            let list: Vec<Value> = r
                .draft
                .rules()
                .into_iter()
                .map(|rv| {
                    let i = rv.index;
                    let mut v = serde_json::to_value(&rv).unwrap_or_default();
                    v["matched"] = json!(r.sim_matched.contains(&i));
                    v["decidesZone"] = json!(r.sim_decided.zone == Some(i));
                    v["decidesTimeout"] = json!(r.sim_decided.timeout == Some(i));
                    v["decidesSilent"] = json!(r.sim_decided.silent == Some(i));
                    v["hasError"] = json!(r.error_idx == Some(i));
                    v
                })
                .collect();
            (json!(list).to_string(), r.draft.rule_count() as i32, json!(r.draft.zone_names()).to_string(), r.draft.is_dirty())
        };
        self.as_mut().set_rules_json(QString::from(&json));
        self.as_mut().set_rules_count(count);
        self.as_mut().set_draft_zones_json(QString::from(&zones));
        let focus = self.as_ref().rust().draft.focus_view().to_string();
        self.as_mut().set_focus_draft_json(QString::from(&focus));
        let settings = self.as_ref().rust().draft.settings_view().to_string();
        self.as_mut().set_settings_json(QString::from(&settings));
        self.as_mut().set_dirty(dirty);
        self.refresh_apps_view();
    }

    /// The apps list as the UI shows it: what was found, plus how this draft treats each app.
    pub(crate) fn refresh_apps_view(mut self: Pin<&mut Self>) {
        let json = {
            let me = self.as_ref();
            let r = me.rust();
            let list: Vec<Value> = r
                .apps
                .iter()
                .map(|a| {
                    let mut v = serde_json::to_value(a).unwrap_or_default();
                    v["routing"] = serde_json::to_value(r.draft.app_routing(&a.match_field, &a.match_value)).unwrap_or_default();
                    // each kind of notification, with how the draft treats it and the match pairs that select it
                    let kinds: Vec<Value> = a
                        .types
                        .iter()
                        .map(|t| {
                            let mut pairs: Vec<(&str, &str)> = vec![(a.match_field.as_str(), a.match_value.as_str())];
                            if !t.category.is_empty() {
                                pairs.push(("category", t.category.as_str()));
                            }
                            pairs.push(("channel", t.channel.as_str()));
                            let mut kv = serde_json::to_value(t).unwrap_or_default();
                            kv["routing"] = serde_json::to_value(r.draft.routing_for(&pairs)).unwrap_or_default();
                            kv["matches"] = json!(pairs);
                            kv
                        })
                        .collect();
                    v["kinds"] = json!(kinds);
                    v
                })
                .collect();
            json!(list).to_string()
        };
        self.as_mut().set_apps_json(QString::from(&json));
    }
}

/// Count `values`, drop empties, and keep the `cap` most frequent (ties alphabetical).
fn top(values: impl Iterator<Item = String>, cap: usize) -> Vec<String> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for v in values.map(|v| v.trim().to_string()).filter(|v| !v.is_empty() && v.chars().count() <= 60) {
        *counts.entry(v).or_default() += 1;
    }
    let mut all: Vec<(String, usize)> = counts.into_iter().collect();
    all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    all.into_iter().take(cap).map(|(v, _)| v).collect()
}

impl qobject::PanelModel {
    pub fn rule_suggestions(self: Pin<&mut Self>) -> QString {
        let me = self.as_ref();
        let r = me.rust();
        let h = &r.history_all;
        let app_names = top(h.iter().map(|e| e.app.clone()).chain(r.apps.iter().map(|a| a.name.clone())), 60);
        let desktop = top(r.apps.iter().map(|a| a.desktop_entry.clone()), 60);
        let origins = top(h.iter().map(|e| e.origin.clone()), 40);
        let titles = top(h.iter().map(|e| e.summary.clone()), 30);
        let categories = top(r.apps.iter().flat_map(|a| a.types.iter().map(|t| t.category.clone())), 40);
        QString::from(
            &json!({
                "appName": app_names, "desktopEntry": desktop, "origin": origins, "summary": titles, "body": [],
                "urgency": ["low", "normal", "critical"], "category": categories, "channel": ["app", "web", "portal"]
            })
            .to_string(),
        )
    }
}
