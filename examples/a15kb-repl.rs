use anyhow::{bail, Context};

fn parse_fan_state(s: &str) -> Option<a15kb::FanState> {
    if s == "quiet" {
        Some(a15kb::FanState::Quiet)
    } else if s == "normal" {
        Some(a15kb::FanState::Normal)
    } else if s == "aggressive" {
        Some(a15kb::FanState::Aggressive)
    } else if let Some(pcnt) = s.strip_prefix("fixed ") {
        let pcnt: f32 = pcnt.parse().ok()?;
        let pcnt = a15kb::Percent::try_from(pcnt).ok()?;
        Some(a15kb::FanState::Fixed(pcnt))
    } else {
        None
    }
}

pub fn main() -> Result<(), anyhow::Error> {
    let mut args = std::env::args();
    let socket_name = match args.nth(1).as_deref() {
        Some("--socket-name") => args
            .next()
            .context("--socket-name must be followed by a socket name")?,
        Some(_) => bail!("unknown option"),
        None => a15kb::DEFAULT_SOCKET_NAME.to_owned(),
    };

    let mut cxn = a15kb::Connection::new(&socket_name)
        .context("failed to connect to socket, is the server running?")?;

    for line in std::io::stdin().lines() {
        let line = line.unwrap();
        if line == "thermal-info" {
            match cxn.thermal_info() {
                Ok(info) => println!("{info:?}"),
                Err(err) => println!("error: {err}"),
            }
        } else if let Some(state) = line.strip_prefix("set-fan-state ") {
            let fan_state = match parse_fan_state(state) {
                Some(fan_state) => fan_state,
                None => {
                    eprintln!("unknown fan state");
                    continue;
                }
            };
            match cxn.set_fan_state(fan_state) {
                Ok(a15kb::FanChangeResponse::Success) => println!("success"),
                Ok(a15kb::FanChangeResponse::UnsafeSpeed(allowed)) => {
                    println!(
                        "unsafe speed specified. allowed: {}..={}",
                        allowed.start(),
                        allowed.end()
                    )
                }
                Err(err) => println!("error: {err}"),
            }
        } else {
            eprintln!("unknown command")
        }
    }

    Ok(())
}
