//! `notify update`: install changed daemon files, smoke-tested first and rolled back if notifd will not start.

use super::*;

fn merge_new_defaults(src: &str, dst: &Path) -> Vec<String> {
    let (Ok(s), Ok(dtext)) = (serde_json::from_str::<Value>(src), fs::read_to_string(dst)) else { return vec![] };
    let Ok(mut d) = serde_json::from_str::<Value>(&dtext) else { return vec![] };
    let (Some(so), Some(dobj)) = (s.as_object(), d.as_object_mut()) else { return vec![] };
    let added: Vec<String> = so.keys().filter(|k| !dobj.contains_key(*k)).cloned().collect();
    if !added.is_empty() {
        for k in &added {
            dobj.insert(k.clone(), so[k].clone());
        }
        let _ = atomic_write(dst, &(serde_json::to_string_pretty(&d).unwrap_or_default() + "\n"));
    }
    added
}

pub fn update(ctx: &Ctx) -> Result<()> {
    let p = paths();
    if !p.takeover_state.exists() {
        bail!("takeover is not installed; run 'notify takeover' first");
    }
    let pl = Payload::load(ctx.payload_dir.as_deref())?;
    let out: Out = ctx.out;
    out.text(&format!("  new version: {}", pl.origin));
    if !ctx.no_systemctl {
        out.text("  smoke-testing the new version on a private bus (your live notifications are not touched)...");
        let rules_for_test = fs::read_to_string(&p.rules).unwrap_or_else(|_| pl.rules.clone());
        if !preflight(&pl, &rules_for_test) {
            bail!("the new version failed its smoke test, so NOTHING was changed. Fix it, or run `notify dev` to see the errors.");
        }
        out.text("  smoke test passed");
    }
    // keep the working version so a bad update can be undone
    fs::create_dir_all(&p.prev)?;
    for n in payload::file_names() {
        if p.payload.join(n).exists() {
            fs::copy(p.payload.join(n), p.prev.join(n))?;
        }
    }
    pl.write_files(&p.payload)?;
    for (n, _) in &pl.files {
        out.text(&format!("  updated {n}"));
    }
    for (path, text) in [(&p.unit, unit_text(&p.payload)), (&p.fallback_unit, fallback_unit_text()), (&p.dbus, dbus_text().to_string())] {
        if fs::read_to_string(path).ok().as_deref() != Some(&text) {
            atomic_write(path, &text)?;
            out.text(&format!("  refreshed {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("")));
        }
    }
    let added = merge_new_defaults(&pl.rules, &p.rules);
    if !added.is_empty() {
        out.text(&format!("  added new settings to your rules.json (existing values untouched): {}", added.join(", ")));
    }
    if ctx.no_systemctl {
        out.result(true, "");
        return Ok(());
    }
    systemctl(ctx, &["daemon-reload"]);
    systemctl(ctx, &["restart", "notifd.service"]);
    out.text("  restarted notifd.service (notifications pause for about a second)");
    if healthy(Some(&p.takeover_state), Duration::from_secs(15)) {
        out.result(true, "  health check passed");
        return Ok(());
    }
    if !p.takeover_state.exists() {
        bail!("notifd kept crashing and the automatic fallback has handed notifications back to Plasma. Fix the problem, then re-enable with: notify takeover --now");
    }
    out.text("  ! notifd did not come back healthy; rolling back to the previous version");
    for n in payload::file_names() {
        if p.prev.join(n).exists() {
            fs::copy(p.prev.join(n), p.payload.join(n))?;
        }
    }
    systemctl(ctx, &["reset-failed", "notifd.service"]);
    systemctl(ctx, &["restart", "notifd.service"]);
    let back = healthy(None, Duration::from_secs(15));
    bail!("the update was rolled back: {}", if back { "notifd is healthy again on the previous version" } else { "notifd is STILL NOT HEALTHY - run: notify restore" })
}
