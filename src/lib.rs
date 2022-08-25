#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as native;

pub use native::{Controller, Error};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum FanMode {
    Quiet,
    Normal,
    Gaming,
    Custom(f32),
}
impl std::fmt::Display for FanMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quiet => write!(f, "quiet"),
            Self::Normal => write!(f, "normal"),
            Self::Gaming => write!(f, "gaming"),
            Self::Custom(pcnt) => write!(f, "fixed ({:.2}%)", pcnt * 100.0),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Command {
    SetFanMode(FanMode),
    GetFanMode,
    GetFanRpm,
    Close,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Response {
    FanRpm((u16, u16)),
    FanMode(FanMode),
    GenericSuccess,
    Failure(String), // followed by error string
    Skipped,         // i.e. unknown command
}
pub const SERVER_NAME: &'static str = "@/mjguynn/a15kb";
