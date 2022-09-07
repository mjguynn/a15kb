#![cfg(target_os = "linux")]
use anyhow::{bail, Error};

/// Runs the server.
/// Accepted args:
/// - `--replace`: Replaces the running a15kb server. (Untested)
pub fn main() -> Result<(), Error> {
    let mut args = std::env::args();
    let replace = match args.nth(1).as_deref() {
        Some("--replace") => true,
        Some(_) => bail!("unknown argument"),
        None => false,
    };
    let cfg = a15kb::ServerCfg { replace };
    a15kb::run_server(&cfg)
}
