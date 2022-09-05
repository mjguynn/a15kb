use a15kb::{Client, FanMode};
use anyhow::{bail, Context};

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
            _ => println!("unknown command"),
        };
    }

    Ok(())
}
