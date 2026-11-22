//! uQ's log format and crash records, from uQ's `native/uq-diag` (`uq_diag.rs`, vendored unchanged:
//! copy a new release over it, `scripts/check-uq-diag.sh` checks it). noti-fi's codes are in
//! `uq/diag/noti-fi.toml`.

#[rustfmt::skip]
mod uq_diag;

pub use uq_diag::*;
