#![cfg(target_os = "linux")]
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::RangeInclusive;
use std::process::Command;

use super::FanMode;

/// The maximum integer speed of the fan. [Source.][source]
///
/// [source]: https://github.com/tangalbert919/p37-ec-aero-15/blob/master/Aero%2015%20Fan%20Control%20Registers.md#custom-fan-mode-auto-maximum
const HW_MAX_FAN_SPEED: u8 = 229;

#[derive(Debug)]
pub struct Error {
    inner: ErrorKind,
}
#[derive(Debug)]
enum ErrorKind {
    EcWriteError {
        offset: u64,
        source: std::io::Error,
    },
    EcReadError {
        offset: u64,
        source: std::io::Error,
    },
    EcAccessError {
        source: std::io::Error,
    },
    OobFanSpeed {
        speed: f32,
        accepted: RangeInclusive<f32>,
    },
    InvalidBit(u8),
    InvalidHwState,
    NoEcSys,
}
use ErrorKind as EK;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            EK::EcWriteError { offset, source } => {
                write!(f, "couldn't write to embedded controller at offset {offset:02X}. caused by: {source}")
            }
            EK::EcReadError { offset, source } => {
                write!(f, "couldn't read from embedded controller at offset {offset:02X}. caused by: {source}")
            }
            EK::EcAccessError { source } => {
                write!(
                    f,
                    "couldn't access embedded controller. caused by: {source}"
                )
            }
            EK::OobFanSpeed { speed, accepted } => {
                write!(
                    f,
                    "unacceptable fan speed {:.2}% specified (valid range: {:.2}%={:.2}%)",
                    speed * 100.0,
                    accepted.start() * 100.0,
                    accepted.end() * 100.0
                )
            }
            EK::InvalidBit(bit) => {
                write!(f, "invalid bit specified (#{bit})")
            }
            EK::InvalidHwState => {
                write!(f, "the hardware is in an invalid state")
            }
            EK::NoEcSys => {
                write!(f, "failed to load ec_sys kernel module with modprobe")
            }
        }
    }
}
impl std::error::Error for Error {}
impl From<EK> for Error {
    fn from(ek: EK) -> Self {
        Self { inner: ek }
    }
}
type Result<T> = std::result::Result<T, Error>;

fn validate_fan_speed(speed: f32) -> Result<()> {
    const MIN_SAFE_FAN_SPEED: f32 = 0.3;
    let accepted = MIN_SAFE_FAN_SPEED..=1.0;
    if !accepted.contains(&speed) {
        Err(EK::OobFanSpeed { speed, accepted }.into())
    } else {
        Ok(())
    }
}
fn cvt_fan_speed(speed: f32) -> Result<u8> {
    validate_fan_speed(speed)?;
    let int_speed = (HW_MAX_FAN_SPEED as f32 * speed).ceil() as u8;
    assert!(int_speed <= HW_MAX_FAN_SPEED);
    Ok(int_speed)
}
fn ivt_fan_speed(int_speed: u8) -> Result<f32> {
    let speed = int_speed as f32 / HW_MAX_FAN_SPEED as f32;
    validate_fan_speed(speed)?;
    Ok(speed)
}
pub struct Controller {
    ec_handle: File,
}
impl Controller {
    fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        self.ec_handle
            .seek(SeekFrom::Start(offset))
            .map_err(|source| EK::EcAccessError { source })?;
        let num_read = self
            .ec_handle
            .read(buffer)
            .map_err(|source| EK::EcReadError { offset, source })?;
        if num_read != buffer.len() {
            Err(EK::EcReadError {
                offset,
                source: std::io::ErrorKind::Other.into(),
            }
            .into())
        } else {
            Ok(())
        }
    }
    fn write_bytes(&mut self, offset: u64, buffer: &[u8]) -> Result<()> {
        let mut ex = || {
            self.ec_handle.seek(SeekFrom::Start(offset))?;
            let num_written = self.ec_handle.write(buffer)?;
            if num_written != buffer.len() {
                return Err(std::io::ErrorKind::Other.into());
            }
            Ok(())
        };
        ex().map_err(|source| EK::EcWriteError { offset, source }.into())
    }
    fn read_bit(&mut self, offset: u64, bit: u8) -> Result<bool> {
        let mut byte = 0u8;
        self.read_bytes(offset, std::slice::from_mut(&mut byte))?;
        let shifted = byte.checked_shr(bit.into()).ok_or(EK::InvalidBit(bit))?;
        let extracted = shifted & 1;
        Ok(extracted != 0)
    }
    fn write_bit(&mut self, offset: u64, bit: u8, val: bool) -> Result<()> {
        let mut byte = 0u8;
        self.read_bytes(offset, std::slice::from_mut(&mut byte))?;
        let shifted = 1u8.checked_shl(bit.into()).ok_or(EK::InvalidBit(bit))?;
        let changed = if val { byte | shifted } else { byte & !shifted };
        self.write_bytes(offset, std::slice::from_ref(&changed))
    }
    fn set_quiet_fans(&mut self, val: bool) -> Result<()> {
        self.write_bit(0x08, 6, val)
    }
    fn get_quiet_fans(&mut self) -> Result<bool> {
        self.read_bit(0x08, 6)
    }
    fn set_gaming_fans(&mut self, val: bool) -> Result<()> {
        self.write_bit(0x0C, 4, val)
    }
    fn get_gaming_fans(&mut self) -> Result<bool> {
        self.read_bit(0x0C, 4)
    }
    fn set_custom_fans(&mut self, val: Option<f32>) -> Result<()> {
        if let Some(speed) = val {
            let int_speed = cvt_fan_speed(speed)?;
            self.write_bit(0x06, 4, true)?; // custom fixed speed enabled
            self.write_bytes(0xB0, &[int_speed, int_speed])?; // set fan 0 & 1
        } else {
            self.write_bit(0x06, 4, false)?;
        }
        Ok(())
    }
    fn get_custom_fans(&mut self) -> Result<Option<f32>> {
        if self.read_bit(0x06, 4)? {
            let mut int_speeds = [0u8, 0u8];
            self.read_bytes(0xB0, &mut int_speeds)?;
            let speed_0 = ivt_fan_speed(int_speeds[0])?;
            let speed_1 = ivt_fan_speed(int_speeds[1])?;
            Ok(Some((speed_0 + speed_1) * 0.5))
        } else {
            Ok(None)
        }
    }
    pub fn set_fan_mode(&mut self, fm: FanMode) -> Result<()> {
        match fm {
            FanMode::Quiet => {
                self.set_quiet_fans(true)?;
                self.set_gaming_fans(false)?;
                self.set_custom_fans(None)
            }
            FanMode::Normal => {
                self.set_quiet_fans(false)?;
                self.set_gaming_fans(false)?;
                self.set_custom_fans(None)
            }
            FanMode::Gaming => {
                self.set_quiet_fans(false)?;
                self.set_gaming_fans(true)?;
                self.set_custom_fans(None)
            }
            FanMode::Custom(pcnt) => {
                self.set_quiet_fans(false)?;
                self.set_gaming_fans(false)?;
                self.set_custom_fans(Some(pcnt))
            }
        }
    }
    pub fn get_fan_mode(&mut self) -> Result<FanMode> {
        let quiet = self.get_quiet_fans()?;
        let gaming = self.get_gaming_fans()?;
        let custom = self.get_custom_fans()?;
        match (quiet, gaming, custom) {
            (true, false, None) => Ok(FanMode::Quiet),
            (false, false, None) => Ok(FanMode::Normal),
            (false, true, None) => Ok(FanMode::Gaming),
            (_, _, Some(pcnt)) => Ok(FanMode::Custom(pcnt)),
            (true, true, None) => Err(EK::InvalidHwState.into()),
        }
    }
    pub fn get_fan_rpm(&mut self) -> Result<(u16, u16)> {
        let mut rpm0 = [0u8, 0u8];
        self.read_bytes(0xFC, &mut rpm0)?;
        let rpm0 = u16::from_be_bytes(rpm0);
        let mut rpm1 = [0u8, 0u8];
        self.read_bytes(0xFE, &mut rpm1)?;
        let rpm1 = u16::from_be_bytes(rpm1);
        Ok((rpm0, rpm1))
    }
    pub fn new() -> Result<Self> {
        // Load ec_sys kernel module
        let status = Command::new("modprobe")
            .arg("ec_sys")
            .arg("write_support=1")
            .status();
        match status {
            Ok(e) if e.success() => (),
            _ => return Err(EK::NoEcSys.into()),
        }

        // Open handle to embedded controller
        let ec_handle = File::options()
            .read(true)
            .write(true)
            .open("/sys/kernel/debug/ec/ec0/io");
        match ec_handle {
            Ok(ec_handle) => Ok(Self { ec_handle }),
            Err(e) => Err(EK::EcAccessError { source: e }.into()),
        }
    }
}
