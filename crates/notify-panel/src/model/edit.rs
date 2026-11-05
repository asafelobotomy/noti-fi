//! The panel's editing commands (rules, zones, focus modes, apps, saving): thin wrappers that change the draft, then re-validate it.
//! A child module of `model`, so it can reach the model's private state; kept apart to keep each file small.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use notify_apps::Roots;
use notify_panel_core::worker::Cmd;

use super::qobject;
use super::*;

impl qobject::PanelModel {
    pub(super) fn idx(i: i32) -> Option<usize> {
        usize::try_from(i).ok()
    }

    pub fn rule_add(mut self: Pin<&mut Self>) -> i32 {
        let n = self.as_mut().rust_mut().draft.add_rule();
        self.after_edit();
        n as i32
    }

    pub fn rule_delete(mut self: Pin<&mut Self>, index: i32) {
        if let Some(i) = Self::idx(index) {
            self.as_mut().rust_mut().draft.delete_rule(i);
            self.after_edit();
        }
    }

    pub fn rule_move(mut self: Pin<&mut Self>, index: i32, delta: i32) -> i32 {
        let Some(i) = Self::idx(index) else { return index };
        let n = self.as_mut().rust_mut().draft.move_rule(i, delta);
        self.after_edit();
        n as i32
    }

    pub fn rule_duplicate(mut self: Pin<&mut Self>, index: i32) -> i32 {
        let Some(i) = Self::idx(index) else { return index };
        let n = self.as_mut().rust_mut().draft.duplicate_rule(i);
        self.after_edit();
        n.map(|n| n as i32).unwrap_or(index)
    }

    pub fn rule_set_name(mut self: Pin<&mut Self>, index: i32, name: &QString) {
        if let Some(i) = Self::idx(index) {
            self.as_mut().rust_mut().draft.set_name(i, &name.to_string());
            self.after_edit();
        }
    }

    pub fn rule_set_match(mut self: Pin<&mut Self>, index: i32, field: &QString, pattern: &QString) {
        if let Some(i) = Self::idx(index) {
            self.as_mut().rust_mut().draft.set_match(i, &field.to_string(), &pattern.to_string());
            self.after_edit();
        }
    }

    pub fn rule_set_zone(mut self: Pin<&mut Self>, index: i32, zone: &QString) {
        if let Some(i) = Self::idx(index) {
            let z = zone.to_string();
            self.as_mut().rust_mut().draft.set_zone(i, Some(z.as_str()));
            self.after_edit();
        }
    }

    pub fn rule_set_timeout(mut self: Pin<&mut Self>, index: i32, text: &QString) {
        if let Some(i) = Self::idx(index) {
            let t = text.to_string();
            let t = t.trim();
            let ms = if t.is_empty() { Some(None) } else { t.parse::<f64>().ok().filter(|n| *n >= 0.0).map(Some) };
            if let Some(ms) = ms {
                self.as_mut().rust_mut().draft.set_timeout(i, ms);
                self.after_edit();
            }
        }
    }

    pub fn rule_set_silent(mut self: Pin<&mut Self>, index: i32, silent: bool) {
        if let Some(i) = Self::idx(index) {
            self.as_mut().rust_mut().draft.set_silent(i, silent);
            self.after_edit();
        }
    }

    pub fn sim_set(mut self: Pin<&mut Self>, field: &QString, value: &QString) {
        if self.as_mut().rust_mut().sim.set(&field.to_string(), &value.to_string()) {
            self.after_edit();
        }
    }

    pub(super) fn refuse(mut self: Pin<&mut Self>, why: &str) {
        self.as_mut().command_failed(&QString::from(why));
    }

    pub fn setting_set(mut self: Pin<&mut Self>, path: &QString, text: &QString) {
        let res = self.as_mut().rust_mut().draft.set_setting(&path.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => {
                self.as_mut().refuse(&why);
                self.refresh_rules_view(); // put the field back to what the draft really holds
            }
        }
    }

    pub fn zone_add(mut self: Pin<&mut Self>) -> QString {
        let id = self.as_mut().rust_mut().draft.add_zone();
        self.after_edit();
        QString::from(&id)
    }

    pub fn zone_delete(mut self: Pin<&mut Self>, id: &QString) {
        let res = self.as_mut().rust_mut().draft.delete_zone(&id.to_string());
        match res {
            Ok(_) => self.after_edit(),
            Err(why) => self.refuse(&why),
        }
    }

    pub fn set_focus(self: Pin<&mut Self>, mode: &QString) {
        self.send(Cmd::Focus(mode.to_string()));
    }

    pub fn focus_add(mut self: Pin<&mut Self>) -> QString {
        let n = self.as_mut().rust_mut().draft.add_focus_mode();
        self.after_edit();
        QString::from(&n)
    }

    pub fn focus_delete(mut self: Pin<&mut Self>, name: &QString) {
        let res = self.as_mut().rust_mut().draft.delete_focus_mode(&name.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => self.refuse(&why),
        }
    }

    pub fn focus_set(mut self: Pin<&mut Self>, name: &QString, field: &QString, text: &QString) {
        let res = self.as_mut().rust_mut().draft.set_focus_field(&name.to_string(), &field.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => {
                self.as_mut().refuse(&why);
                self.refresh_rules_view();
            }
        }
    }

    pub fn zone_style(mut self: Pin<&mut Self>, id: &QString, key: &QString, text: &QString) {
        let res = self.as_mut().rust_mut().draft.set_zone_style(&id.to_string(), &key.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => {
                self.as_mut().refuse(&why);
                self.refresh_rules_view();
            }
        }
    }

    pub fn rule_set_style(mut self: Pin<&mut Self>, index: i32, key: &QString, text: &QString) {
        if let Some(i) = Self::idx(index) {
            let res = self.as_mut().rust_mut().draft.set_rule_style(i, &key.to_string(), &text.to_string());
            match res {
                Ok(()) => self.after_edit(),
                Err(why) => {
                    self.as_mut().refuse(&why);
                    self.refresh_rules_view();
                }
            }
        }
    }

    pub fn zone_place(mut self: Pin<&mut Self>, id: &QString, x: f64, y: f64) {
        let res = self.as_mut().rust_mut().draft.set_zone_position(&id.to_string(), x, y);
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => self.refuse(&why),
        }
    }

    pub fn zone_set(mut self: Pin<&mut Self>, id: &QString, field: &QString, text: &QString) {
        let res = self.as_mut().rust_mut().draft.set_zone_field(&id.to_string(), &field.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => self.refuse(&why),
        }
    }

    pub fn apps_rescan(mut self: Pin<&mut Self>) {
        let token = {
            let mut r = self.as_mut().rust_mut();
            r.apps_token += 1;
            r.apps_token
        };
        self.as_mut().set_apps_scanning(true);
        self.as_ref().send_cmd(Cmd::ScanApps { token, roots: Roots::system() });
    }

    pub fn app_set(mut self: Pin<&mut Self>, match_field: &QString, match_value: &QString, label: &QString, what: &QString, text: &QString) {
        let res = self.as_mut().rust_mut().draft.set_app_routing(&label.to_string(), &match_field.to_string(), &match_value.to_string(), &what.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => {
                self.as_mut().refuse(&why);
                self.refresh_apps_view();
            }
        }
    }

    pub fn app_set_kind(mut self: Pin<&mut Self>, matches_json: &QString, label: &QString, what: &QString, text: &QString) {
        let pairs: Vec<(String, String)> = match serde_json::from_str::<Vec<(String, String)>>(&matches_json.to_string()) {
            Ok(p) => p,
            Err(_) => return self.refuse("could not tell which notifications to change"),
        };
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(f, v)| (f.as_str(), v.as_str())).collect();
        let res = self.as_mut().rust_mut().draft.set_routing(&label.to_string(), &refs, &what.to_string(), &text.to_string());
        match res {
            Ok(()) => self.after_edit(),
            Err(why) => {
                self.as_mut().refuse(&why);
                self.refresh_apps_view();
            }
        }
    }

    pub fn app_flatpak(mut self: Pin<&mut Self>, app_id: &QString, mode: &QString) {
        let allow = match mode.to_string().as_str() {
            "allow" => Some(true),
            "deny" => Some(false),
            "reset" => None,
            other => return self.refuse(&format!("unknown Flatpak mode '{other}'")),
        };
        let token = {
            let mut r = self.as_mut().rust_mut();
            r.apps_token += 1;
            r.apps_token
        };
        self.as_mut().set_apps_scanning(true);
        self.as_ref().send_cmd(Cmd::FlatpakPermission { token, roots: Roots::system(), app: app_id.to_string(), allow });
    }

    pub fn rules_preview(self: Pin<&mut Self>) {
        let config = self.as_ref().rust().draft.value().clone();
        self.send(Cmd::Preview { config, seconds: 6 });
    }

    pub fn rules_end_preview(self: Pin<&mut Self>) {
        self.send(Cmd::EndPreview);
    }

    pub fn rules_reload(mut self: Pin<&mut Self>) {
        let raw = {
            let me = self.as_ref();
            let r = me.rust();
            if r.conflict_disk.is_empty() { r.latest_raw.clone() } else { r.conflict_disk.clone() }
        };
        if let Ok(d) = Draft::from_raw(&raw) {
            let mut r = self.as_mut().rust_mut();
            r.draft = d;
            r.conflict_disk.clear();
            self.as_mut().set_conflict(false);
            self.as_mut().set_external_changed(false);
            self.as_mut().set_save_message(QString::from(""));
            self.after_edit();
        }
    }

    pub fn rules_save(self: Pin<&mut Self>, force: bool) {
        let (text, expected_base) = {
            let me = self.as_ref();
            let r = me.rust();
            (r.draft.to_text(), r.draft.base_raw().to_string())
        };
        self.send(Cmd::Save { path: draft::rules_path(), text, expected_base, force });
    }

}
