#![cfg(target_os = "linux")]
//! Unofficial hardware controller for GIGABYTE AERO 15 KB laptops.
//!
//! # Overview
//! [`a15kb`] is implemented using a client-server model. The server is
//! launched with root privileges. It loads the `ec_sys` kernel module to
//! communicate with the laptop's embedded controller and opens a D-Bus
//! connection. Clients run at any privilege level. They connect to the
//! socket, submit requests to the server, and receive responses.
//!
//! # Notes
//! Running multiple servers at once probably isn't a good idea. I'm unsure
//! whether concurrent writes to the embedded controller are serialized by
//! the kernel or whether they cause a data race (which could be
//! disasterous). My bet's on serialization, but I'm too afraid to test it.
//!
//! I'd like to support Windows, however...
//!     - You can't access the embedded controller in Windows without a kernel
//!       driver. Most people seem to use [WinRing0x64.sys], but it's flagged
//!       as malware by many AV vendors. (Hell, maybe it is malware. I can't
//!       find the source for it.)
//!     - [aeroctl] is an existing solution for controlling Aero fans on
//!       Windows. (They hijack the existing Gigabyte ACPI WMI driver instead
//!       of installing their own kernel driver, which is a much more clever
//!       way to gain fan access.)
//!
//! [aeroctl]: https://gitlab.com/wtwrp/aeroctl/
//! [WinRing0x64.sys]: https://github.com/Soberia/EmbeddedController/blob/main/WinRing0x64.sys

use dbus::blocking::{Connection, Proxy};
use std::fmt::{Display, Formatter};
use std::ops::RangeInclusive;
use std::time::Duration;

mod ec;
mod server;

#[allow(clippy::type_complexity)]
#[allow(clippy::needless_borrow)]
mod client_generated {
    include! { concat!(env!("OUT_DIR"), "/client_generated.rs") }
}
use client_generated::ComOffbyondA15kbController1;

pub use server::run_server;
pub use server::ServerCfg;

/// The name of the service, which always resides on the system bus.
pub const BUS_NAME: &str = "com.offbyond.a15kb";

/// Laptop fan mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanMode {
    /// Quiet fans. May temporarily turn off the fan, thermal-throttle the
    /// CPU, and disable Turboboost.
    Quiet,
    /// Normal fans.
    Normal,
    /// Aggressive "gaming" fans.
    Gaming,
    /// A fixed, user-controlled fan speed.
    Fixed,
}

impl FanMode {
    /// Converts a numeric discriminant into its corresponding
    /// [`FanMode`]. Returns [`None`] in the case of an unrecognized
    /// discriminant. The valid discriminants are:
    /// - `0`: [Quiet](`self::FanMode#variant.Quiet`)
    /// - `1`: [Normal](`self::FanMode#variant.Normal`)
    /// - `2`: [Gaming](`self::FanMode#variant.Gaming`)
    /// - `3`: [Fixed](`self::FanMode#variant.Fixed`)
    const fn from_discriminant(discriminant: u8) -> Option<Self> {
        match discriminant {
            0 => Some(Self::Quiet),
            1 => Some(Self::Normal),
            2 => Some(Self::Gaming),
            3 => Some(Self::Fixed),
            _ => None,
        }
    }
    /// The inverse of [from_discriminant][`FanMode#method.from_discriminant`]
    const fn to_discriminant(self) -> u8 {
        match self {
            Self::Quiet => 0,
            Self::Normal => 1,
            Self::Gaming => 2,
            Self::Fixed => 3,
        }
    }
}

/// The current thermal state of the system.
#[derive(Debug)]
pub struct ThermalInfo {
    /// The CPU temperature, in Celcius.
    pub temp_cpu: Celcius,

    /// The GPU temperature, in Celcius. This is 0 if the GPU is currently
    /// powered off.
    pub temp_gpu: Celcius,

    /// The RPM of the left and right fans, respectively.
    pub fan_rpm: (u16, u16),
}

/// Convenience alias.
type ClientResult<T> = Result<T, dbus::Error>;

/// Represents a client connection to the a15kb server.
/// All method calls are blocking.
pub struct Client {
    conn: Connection,
}
impl Client {
    /// Creates a new client which lies dormant on the system bus.
    pub fn new() -> ClientResult<Self> {
        Ok(Self {
            conn: Connection::new_system()?,
        })
    }

    fn with_proxy<F, T>(&self, mut f: F) -> ClientResult<T>
    where
        F: FnMut(&Proxy<&'_ Connection>) -> ClientResult<T>,
    {
        const TIMEOUT: Duration = Duration::from_millis(1000);
        let proxy = self
            .conn
            .with_proxy(BUS_NAME, "/com/offbyond/a15kb/Controller1", TIMEOUT);
        f(&proxy)
    }

    /// Returns the server's allowable fan speeds.
    pub fn allowed_fixed_fan_speeds(&self) -> ClientResult<RangeInclusive<Percent>> {
        self.with_proxy(|proxy| {
            let (min, max) = proxy.allowed_fixed_fan_speeds()?;
            let min = Percent::try_from(min)
                .map_err(|_| dbus::Error::new_failed("invalid min fan speed"))?;
            let max = Percent::try_from(max)
                .map_err(|_| dbus::Error::new_failed("invalid max fan speed"))?;
            if min > max {
                Err(dbus::Error::new_failed("reversed speed range"))
            } else {
                Ok(min..=max)
            }
        })
    }

    /// Returns the system's current thermal information.
    pub fn thermal_info(&self) -> ClientResult<ThermalInfo> {
        self.with_proxy(|proxy| {
            let (temp_cpu, temp_gpu, fan_rpm) = proxy.get_thermal_info()?;
            // let fixed_fan_speed = Percent::try_from(fixed_fan_speed)
            //     .map_err(|_| dbus::Error::new_failed("invalid fan speed"))?;
            // let fan_mode = FanMode::from_discriminant(fan_mode);
            Ok(ThermalInfo {
                temp_cpu,
                temp_gpu,
                fan_rpm,
            })
        })
    }

    /// Returns the current fan mode, or `None` if the fan mode is unrecognized.
    pub fn fan_mode(&self) -> ClientResult<Option<FanMode>> {
        self.with_proxy(|proxy| Ok(FanMode::from_discriminant(proxy.fan_mode()?)))
    }

    /// Attempts to set the current fan mode.
    pub fn set_fan_mode(&self, fan_mode: FanMode) -> ClientResult<()> {
        self.with_proxy(|proxy| proxy.set_fan_mode(fan_mode.to_discriminant()))
    }

    /// Returns the current fixed fan speed.
    pub fn fixed_fan_speed(&self) -> ClientResult<Percent> {
        self.with_proxy(|proxy| {
            let fixed_fan_speed = proxy.fixed_fan_speed()?;
            Percent::try_from(fixed_fan_speed)
                .map_err(|_| dbus::Error::new_failed("negative fan speed"))
        })
    }
    /// Attempts to set the fixed fan speed. The specified value should be in
    /// the server's acceptable range, which can be retrieved by calling
    /// [`allowed_fixed_fan_speeds`].
    ///
    /// [`allowed_fixed_fan_speeds`]: self::FanMode#method.allowed_fixed_fan_speeds
    pub fn set_fixed_fan_speed(&self, fixed_fan_speed: Percent) -> ClientResult<()> {
        self.with_proxy(|proxy| proxy.set_fixed_fan_speed(fixed_fan_speed.as_f64()))
    }
}

/// A temperature in degrees Celcius.
pub type Celcius = u8;

/// A newtype wrapper around an `f64` which ensures the wrapped value is positive.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Percent(f64);

impl Percent {
    /// Creates a percent if the given value is positive.
    pub fn new(value: f64) -> Option<Self> {
        value.is_sign_positive().then_some(Self(value))
    }
    /// Returns the wrapped float.
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

impl Display for Percent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.0 * 100.0).fmt(f)?;
        f.write_str("%")
    }
}
impl From<Percent> for f64 {
    fn from(value: Percent) -> Self {
        value.as_f64()
    }
}

/// An error thrown when converting an `f32` to a [`Percent`].
#[derive(Debug)]
pub struct FromPercentError;

impl TryFrom<f64> for Percent {
    type Error = FromPercentError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(FromPercentError {})
    }
}
