//! Handlers for the answers the worker sends back (validation, simulation, saving, apps): they turn a result into property updates.
//! A child module of `model`, so it can reach the model's private state.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use notify_apps::AppEntry;
use notify_panel_core::draft::SaveOutcome;
use notify_panel_core::view;
use notify_protocol::SimResult;
use serde_json::json;

use super::qobject;
use super::*;

impl qobject::PanelModel {
    pub(super) fn on_validated(mut self: Pin<&mut Self>, token: u64, ok: bool, error: String) {
        if token != self.as_ref().rust().validate_token {
            return; // an answer for an older draft
        }
        let idx = self.as_ref().rust().draft.error_index(&error);
        self.as_mut().rust_mut().error_idx = idx;
        self.as_mut().set_draft_valid(ok);
        self.as_mut().set_draft_error(QString::from(&error));
        self.as_mut().set_checking(false);
        self.refresh_rules_view();
    }

    pub(super) fn on_simulated(mut self: Pin<&mut Self>, token: u64, result: Result<Box<SimResult>, String>) {
        if token != self.as_ref().rust().sim_token {
            return;
        }
        match result {
            Ok(r) => {
                let v = view::sim_view(&r);
                let mut me = self.as_mut().rust_mut();
                me.sim_matched = r.matched_idx.clone();
                me.sim_decided = r.decided_idx.clone();
                self.as_mut().set_sim_json(QString::from(&v.to_string()));
            }
            Err(e) => {
                let mut me = self.as_mut().rust_mut();
                me.sim_matched.clear();
                me.sim_decided = DecidedIdx::default();
                self.as_mut().set_sim_json(QString::from(&json!({"ok": false, "error": e}).to_string()));
            }
        }
        self.refresh_rules_view();
    }

    pub(super) fn on_saved(mut self: Pin<&mut Self>, outcome: SaveOutcome, text: String) {
        match outcome {
            SaveOutcome::Saved => {
                self.as_mut().rust_mut().draft.mark_saved(&text);
                self.as_mut().set_conflict(false);
                self.as_mut().set_external_changed(false);
                self.as_mut().set_save_message(QString::from("Saved. notifd has reloaded the rules."));
                self.refresh_rules_view();
            }
            SaveOutcome::Conflict { disk } => {
                self.as_mut().rust_mut().conflict_disk = disk;
                self.as_mut().set_conflict(true);
                self.as_mut().set_save_message(QString::from("rules.json was changed on disk since you started editing; nothing was saved."));
            }
            SaveOutcome::Failed(why) => {
                self.as_mut().set_save_message(QString::from(&why));
                self.as_mut().command_failed(&QString::from(&why));
            }
        }
    }

    pub(super) fn on_apps(mut self: Pin<&mut Self>, token: u64, result: Result<Vec<AppEntry>, String>) {
        if token != self.as_ref().rust().apps_token {
            return;
        }
        self.as_mut().set_apps_scanning(false);
        match result {
            Ok(list) => {
                self.as_mut().rust_mut().apps = list;
                self.refresh_apps_view();
            }
            Err(why) => self.as_mut().command_failed(&QString::from(&why)),
        }
    }
}
