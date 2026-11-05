//! Handing notifications back to Plasma (`notify restore`, and the silent rollback paths).

use super::*;

/// The steps that hand notifications back to Plasma. Used by `notify restore` (reported step by step) and by
/// the rollback paths (run silently).
fn restore_steps(ctx: &Ctx, purge: bool) -> Vec<Step> {
    let p = paths();
    let state = std::rc::Rc::new(read_state(&p).unwrap_or_default());
    let mut steps = Vec::new();
    {
        let c = ctx.clone();
        steps.push(Step::new("systemctl --user disable --now notifd.service", move || {
            systemctl(&c, &["disable", "--now", "notifd.service"]);
            Ok(())
        }));
    }
    {
        let (st, up, fp, dp) = (state.clone(), p.unit.clone(), p.fallback_unit.clone(), p.dbus.clone());
        steps.push(Step::new(format!("remove {}, {} and the D-Bus activation override", file_name(&p.unit), file_name(&p.fallback_unit)), move || {
            for (recorded, default) in [(&st.unit, &up), (&st.fallback_unit, &fp), (&st.dbus, &dp)] {
                let f = if recorded.is_empty() { default.clone() } else { PathBuf::from(recorded) };
                if f.exists() {
                    fs::remove_file(&f)?;
                }
            }
            Ok(())
        }));
    }
    {
        let (c, st) = (ctx.clone(), state.clone());
        steps.push(Step::new("remove the portal routing (if notify created it) and the full-screen KWin script", move || {
            portal_disable(&c)?;
            if st.autodnd && !c.no_systemctl {
                integrations::autodnd_remove();
            }
            // Plasma needs its notification tray item to take notifications back; show it if `notify tray hide` hid it
            if !c.no_systemctl {
                integrations::tray_ensure_shown();
            }
            Ok(())
        }));
    }
    {
        let (c, ts) = (ctx.clone(), p.takeover_state.clone());
        steps.push(Step::new("daemon-reload; if nobody owns the name, restart plasmashell so Plasma takes it back", move || {
            if !c.no_systemctl {
                systemctl(&c, &["daemon-reload"]);
                systemctl(&c, &["reset-failed", "notifd.service"]);
                for _ in 0..10 {
                    // give plasmashell a moment to take the name (its notification applet may just have been shown again)
                    if owner().is_some() {
                        break;
                    }
                    sleep_ms(500);
                }
                if owner().is_none() {
                    systemctl(&c, &["restart", "plasma-plasmashell.service"]);
                }
            }
            if ts.exists() {
                fs::remove_file(&ts)?;
            }
            Ok(())
        }));
    }
    if purge {
        let (payload, prev) = (p.payload.clone(), p.prev.clone());
        steps.push(Step::new(format!("delete daemon files {} (your config in {} is kept)", p.payload.display(), p.rules.display()), move || {
            let _ = fs::remove_dir_all(&payload);
            let _ = fs::remove_dir_all(&prev);
            Ok(())
        }));
    }
    steps
}

/// Hand notifications back without printing anything (rollbacks).
pub fn do_restore(ctx: &Ctx, _quiet: bool, _auto: bool) -> Result<()> {
    for s in restore_steps(ctx, false) {
        if let Some(a) = s.action {
            a()?;
        }
    }
    Ok(())
}

pub fn restore(ctx: &Ctx, dry: bool, purge: bool, auto: bool) -> Result<()> {
    ctx.out.text(&format!("{}restore plan", if dry { "DRY RUN - " } else { "" }));
    let steps = restore_steps(ctx, purge);
    run_plan(&ctx.out, "restore", steps, dry)?;
    if dry {
        ctx.out.result(true, "\nDry run only; nothing was changed.");
        return Ok(());
    }
    ctx.out.result(true, "restored: Plasma owns notifications again");
    if auto && !ctx.no_systemctl {
        for _ in 0..20 {
            if owner().is_some() {
                break;
            }
            sleep_ms(500);
        }
        let _ = Command::new("notify-send")
            .args(["-a", "noti-fi", "-u", "critical", "Notification daemon failed", "notifd kept failing, so notifications were handed back to Plasma. Check the setup, then run: notify takeover --now"])
            .status();
    }
    Ok(())
}

fn file_name(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

