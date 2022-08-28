#![cfg(target_os = "linux")]
use a15kb::*;
use anyhow::{bail, ensure, Context};
use std::fs;
use std::io::ErrorKind as IoErr;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net;
use std::path::PathBuf;
use std::thread;

pub fn main() -> Result<(), anyhow::Error> {
    let mut args = std::env::args();
    let socket_name = match args.nth(1).as_deref() {
        Some("--socket-name") => args
            .next()
            .context("--socket-name must be followed by a socket name")?,
        Some(_) => bail!("unknown argument"),
        None => DEFAULT_SOCKET_NAME.to_string(),
    };

    // Before we do anything else, make sure we're actually running on an Aero 15 KB.
    //
    // This seems a bit silly -- why would you install this if you're not running a supported
    // computer? -- but I'm actually developing this on a persistent USB install, which I could
    // theoretically try to run on another computer in the future.
    //
    // If you're using another Aero model and want to run this, you can remove this check.
    // Caveat emptor.
    let product_name = fs::read_to_string("/sys/class/dmi/id/product_name")
        .context("couldn't retrieve product name")?;
    ensure!(
        product_name == "AERO 15 KB",
        "unsupported hardware ({product_name})"
    );

    // Create the socket directory if it doesn't already exist.
    if let Err(err) = fs::create_dir(SOCKET_DIR) {
        ensure!(
            err.kind() == IoErr::AlreadyExists,
            "couldn't create socket directory"
        );
    }

    // Make sure everyone has R/W permissions for that directory.
    let rw = fs::Permissions::from_mode(0o666);
    fs::set_permissions(SOCKET_DIR, rw).context("couldn't set socket directory permissions")?;

    let mut path = PathBuf::from(SOCKET_DIR);
    path.push(socket_name);

    let listener = net::UnixListener::bind(path).context("couldn't bind listener")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || todo!());
            }
            Err(err) => eprintln!("client connection failed: {err}"),
        }
    }
    Ok(())
}
