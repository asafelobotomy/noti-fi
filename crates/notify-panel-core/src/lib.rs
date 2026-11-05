//! The panel's logic, with no Qt in it: connecting to notifd, turning its state into what the UI shows,
//! and filtering history. The Qt bridge (the `notify-panel` crate) only forwards between this and QML, so everything
//! interesting can be tested without a display.

pub mod draft;
pub mod shell;
pub mod tray;
pub mod view;
pub mod worker;
