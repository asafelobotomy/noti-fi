//! The text of the systemd units and the D-Bus activation file that `notify takeover` installs.

use std::path::Path;

use crate::install::exe_path;

/// How the unit is ordered and started: before plasmashell on Plasma (with a fallback that hands notifications back), or as a
/// plain user service of the graphical session elsewhere (Hyprland, Sway ...), where there is no Plasma to take over from.
fn session_lines(plain: bool) -> (&'static str, &'static str) {
    if plain {
        ("After=graphical-session.target\nPartOf=graphical-session.target\n", "graphical-session.target")
    } else {
        (
            "# Start before plasmashell so we claim the name first; Plasma then runs without it.
After=plasma-kwin_wayland.service
Before=plasma-plasmashell.service
PartOf=graphical-session.target
# If notifd keeps failing to start (5 failed starts within 3 minutes, about a minute of
# trying), hand notifications back to Plasma instead of leaving none.
OnFailure=notifd-fallback.service
",
            "plasma-core.target",
        )
    }
}

/// notifd: `notify serve` owns the notification name and runs Quickshell (the daemon) on a private bus behind it,
/// so it can answer what Quickshell cannot (Inhibit). It stops when Quickshell stops, and systemd starts both again.
pub fn unit_text(payload: &Path, plain: bool) -> String {
    let (order, wanted_by) = session_lines(plain);
    format!(
        "[Unit]
Description=notifd notification daemon (owns org.freedesktop.Notifications)
{order}StartLimitIntervalSec=180
StartLimitBurst=5

[Service]
Type=dbus
BusName=org.freedesktop.Notifications
ExecStart={exe} serve --payload {payload}
Restart=always
RestartSec=1
TimeoutStartSec=12
Slice=session.slice
# notifd needs nothing privileged: it cannot gain privileges, and files it creates (history, state) are private to you.
NoNewPrivileges=yes
UMask=0077
LockPersonality=yes
RestrictSUIDSGID=yes

[Install]
WantedBy={wanted_by}
",
        exe = exe_path(),
        payload = payload.display()
    )
}

/// The job bridge: KDE file operations (Dolphin's copy/move/delete progress) as notifd notifications.
/// Plasma takes org.kde.JobViewServer without allowing replacement, so this must hold it before plasmashell starts.
/// Without Plasma (plain mode) nobody else claims the names, so KDE apps on other desktops report their copies here too.
pub fn jobs_unit_text(plain: bool) -> String {
    let order = if plain { "After=graphical-session.target notifd.service" } else { "After=plasma-kwin_wayland.service notifd.service\nBefore=plasma-plasmashell.service" };
    format!(
        "[Unit]
Description=noti-fi job bridge (shows file copy/move/delete progress through notifd; owns org.kde.JobViewServer)
{order}
PartOf=graphical-session.target

[Service]
# ready once it holds the names, or waits in the queue behind their current owner (on KDE, the shell starts after this)
Type=notify
NotifyAccess=main
ExecStart={} jobs
Restart=on-failure
RestartSec=2
TimeoutStartSec=12
Slice=session.slice
NoNewPrivileges=yes
UMask=0077
LockPersonality=yes
RestrictSUIDSGID=yes

[Install]
WantedBy={}
",
        exe_path(),
        if plain { "graphical-session.target" } else { "plasma-core.target" }
    )
}

pub fn fallback_unit_text() -> String {
    format!(
        "[Unit]
Description=Hand notifications back to Plasma because notifd failed

[Service]
Type=oneshot
ExecStart={} restore --auto
",
        exe_path()
    )
}

pub fn dbus_text() -> &'static str {
    "[D-BUS Service]
Name=org.freedesktop.Notifications
Exec=/usr/bin/false
SystemdService=notifd.service
"
}
