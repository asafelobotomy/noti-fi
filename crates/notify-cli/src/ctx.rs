//! Options shared by every command.

use std::path::PathBuf;

use crate::out::Out;

#[derive(Clone)]
pub struct Ctx {
    pub out: Out,
    /// Talk to the `notify dev` daemon instead of the real one.
    pub dev: bool,
    /// Tests only: skip every systemctl / session-changing call.
    pub no_systemctl: bool,
    /// Use daemon files from this directory instead of the ones embedded in the binary.
    pub payload_dir: Option<PathBuf>,
}
