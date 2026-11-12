//! The plain takeover's wait for whoever holds the notification name (uQ's shell, or another daemon) to let go.

use std::time::Duration;

use crate::sys::Owner;

/// How long a plain `takeover --now` waits for notifd to own the name. With nobody else holding it, notifd takes it at
/// once. A desktop with its own server (uQ) lets go only once it has found notifd's socket, and uQ 0.3.0-rc.2 looks
/// for it at most every 30 s once a session has run a while; mako, dunst or swaync never let go, and the step says so.
pub(crate) fn plain_start_timeout(other: Option<&Owner>) -> Duration {
    Duration::from_secs(if other.is_some() { 45 } else { 10 })
}

/// What the plain takeover prints when someone else owns the notification name.
pub(crate) fn handover_note(o: &Owner) -> String {
    if o.unit == "uq-shell.service" {
        format!("uQ's shell (pid {}) owns notifications; it hands them to notifd once it sees it (waiting up to 45 s)", o.pid)
    } else {
        format!("{} (pid {}) owns notifications; notifd waits behind it until it stops (waiting up to 45 s)", o.comm, o.pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(comm: &str, unit: &str) -> Owner {
        Owner { pid: "42".into(), comm: comm.into(), unit: unit.into() }
    }

    #[test]
    fn waits_longer_when_someone_else_holds_the_name() {
        assert_eq!(plain_start_timeout(None), Duration::from_secs(10));
        // uQ 0.3.0-rc.2 probes for notifd's socket at most every 30 s, then lets go
        assert!(plain_start_timeout(Some(&o("qs", "uq-shell.service"))) > Duration::from_secs(30));
    }

    #[test]
    fn names_uq_by_its_unit() {
        assert!(handover_note(&o("qs", "uq-shell.service")).starts_with("uQ's shell (pid 42)"));
        assert!(handover_note(&o("mako", "")).starts_with("mako (pid 42) owns notifications"));
    }
}
