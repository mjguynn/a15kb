#![cfg(target_os = "linux")]
use anyhow::{bail, Context};
use std::path::Path;

pub fn main() -> Result<(), anyhow::Error> {
    let mut args = std::env::args();
    let socket_name = match args.nth(1).as_deref() {
        Some("--socket-name") => args
            .next()
            .context("--socket-name must be followed by a socket name")?,
        Some(_) => bail!("unknown argument"),
        None => a15kb::DEFAULT_SOCKET_NAME.to_string(),
    };
    a15kb::run_server(Path::new(socket_name.as_str()))
}
