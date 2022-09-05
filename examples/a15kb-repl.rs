use a15kb::{Client, FanMode, Percent};
use anyhow::Context;

fn parse_fan_mode(s: &str) -> Option<FanMode> {
    match s {
        "quiet" => Some(FanMode::Quiet),
        "normal" => Some(FanMode::Normal),
        "gaming" => Some(FanMode::Gaming),
        "fixed" => Some(FanMode::Fixed),
        _ => None,
    }
}

pub fn main() -> Result<(), anyhow::Error> {
    let client = Client::new().context("failed to initialize client")?;
    for line in std::io::stdin().lines() {
        match line.unwrap().as_str() {
            "Quit" => break,
            "AllowedFanSpeeds" => match client.allowed_fan_speeds() {
                Ok(range) => println!("{} to {}", range.start(), range.end()),
                Err(err) => println!("error: {err}"),
            },
            "GetThermalInfo" => match client.get_thermal_info() {
                Ok(info) => println!("{info:?}"),
                Err(err) => println!("error: {err}"),
            },
            a if a.starts_with("SetFanMode ") => {
                let rest = a.strip_prefix("SetFanMode ").unwrap();
                let fan_mode = match parse_fan_mode(rest) {
                    Some(fan_mode) => fan_mode,
                    None => {
                        println!("error: unknown fan mode");
                        continue;
                    }
                };
                match client.set_fan_mode(fan_mode) {
                    Ok(()) => println!("done"),
                    Err(err) => println!("error: {err}"),
                }
            }
            a if a.starts_with("SetFixedFanSpeed ") => {
                let rest = a.strip_prefix("SetFixedFanSpeed ").unwrap();
                let f = match rest.parse::<f64>() {
                    Ok(f) => f,
                    Err(err) => {
                        println!("error: {err}");
                        continue;
                    }
                };
                let fixed_fan_speed = match Percent::try_from(f) {
                    Ok(fixed_fan_speed) => fixed_fan_speed,
                    Err(_) => {
                        println!("error: not a percentage");
                        continue;
                    }
                };
                match client.set_fixed_fan_speed(fixed_fan_speed) {
                    Ok(()) => println!("done"),
                    Err(err) => println!("error: {err}"),
                }
            }
            _ => println!("error: unknown command"),
        };
    }

    Ok(())
}
