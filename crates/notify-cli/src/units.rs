//! The text of the systemd units and the D-Bus activation file that `notify takeover` installs.

use std::path::Path;

use crate::install::exe_path;
use crate::sys::which;

pub fn unit_text(payload: &Path) -> String {
    let qs = which("qs").unwrap_or_else(|| "/usr/bin/qs".into());
    format!(
        "[Unit]
Description=notifd notification daemon (owns org.freedesktop.Notifications)
# Start before plasmashell so we claim the name first; Plasma then runs without it.
After=plasma-kwin_wayland.service
Before=plasma-plasmashell.service
PartOf=graphical-session.target
# If notifd keeps failing to start (5 failed starts within 3 minutes, about a minute of
# trying), hand notifications back to Plasma instead of leaving none.
OnFailure=notifd-fallback.service
StartLimitIntervalSec=180
StartLimitBurst=5

[Service]
Type=dbus
BusName=org.freedesktop.Notifications
ExecStart={qs} -p {payload}
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
WantedBy=plasma-core.target
",
        payload = payload.display()
    )
}

/// The job bridge: KDE file operations (Dolphin's copy/move/delete progress) as notifd notifications.
/// Plasma takes org.kde.JobViewServer without allowing replacement, so this must hold it before plasmashell starts.
pub fn jobs_unit_text() -> String {
    format!(
        "[Unit]
Description=noti-fi job bridge (shows file copy/move/delete progress through notifd; owns org.kde.JobViewServer)
After=plasma-kwin_wayland.service notifd.service
Before=plasma-plasmashell.service
PartOf=graphical-session.target

[Service]
# ready once it holds the names (or is queued behind Plasma), so plasmashell starts after it
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
WantedBy=plasma-core.target
",
        exe_path()
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
