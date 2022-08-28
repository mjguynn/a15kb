#![cfg(target_os = "linux")]
//! Controlling the hardware of a GIGABYTE Aero 15 KB.
//!
//! # Overview
//! [`a15kb`] is implemented using a client-server model. The server is launched with root
//! privileges. It loads the `ec_sys` kernel module to communicate with the laptop's embedded
//! controller, then opens an Unix domain socket within `/var/a15kb/`. Clients run at any privilege
//! level. They connect to the socket, submit requests to the server, and receive responses.
//!
//! # Notes
//! Running multiple servers at once probably isn't a good idea. I'm unsure whether concurrent
//! writes to the embedded controller are serialized by the kernel or whether they cause a data
//! race (which could be disasterous). My bet's on serialization, but I'm too afraid to test it.
//!
//! I'd like to support Windows, however...
//! 	- You can't access the embedded controller in Windows without a kernel driver. Most people
//!		  seem to use [WinRing0x64.sys], but it's flagged as malware by many AV vendors. (Hell,
//!		  maybe it is malware. I can't find the source for it.)
//!		- [aeroctl] is an existing solution for controlling Aero fans on Windows. (They hijack the
//!		  existing Gigabyte ACPI WMI driver instead of installing their own kernel driver, which
//!		  is a much more clever way to gain fan access.)
//!
//! [aeroctl]: https://gitlab.com/wtwrp/aeroctl/
//! [WinRing0x64.sys]: https://github.com/Soberia/EmbeddedController/blob/main/WinRing0x64.sys

use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::os::unix::net;
use std::path::PathBuf;

mod server;

pub use server::run_server;

/// The socket directory. Unlike the socket name, this is static and cannot be changed.
pub const SOCKET_DIR: &'static str = "/run/a15kb";

/// The filename of the listening socket in [`SOCKET_DIR`]. This can be changed by running the
/// server with `--socket-name [name]`.
pub const DEFAULT_SOCKET_NAME: &'static str = "default.sock";

/// Laptop fan mode.
#[derive(Debug, Clone, Copy, PartialEq, Decode, Encode)]
pub enum FanState {
    /// Quiet fans. May temporarily turn off the fan, thermal-throttle the CPU, and disable
    /// Turboboost.
    Quiet,
    /// Normal fans.
    Normal,
    /// Aggressive/"gaming" fans.
    Aggressive,
    /// A fixed, user-controlled fan speed.
    Fixed(Percent),
}

/// The server's response to requesting a change in fan mode.
#[derive(Debug, Decode, Encode)]
pub enum FanChangeResponse {
    /// The fan mode was changed succesfully.
    Success,
    /// The requested fixed fan speed was below the minimum safe speed (returned in payload).
    UnsafeSpeed(Percent),
}

#[derive(Debug, Decode, Encode)]
pub struct ThermalInfo {
    /// The CPU temperature, in Celcius.
    pub temp_cpu: Celcius,

    /// The GPU temperature, in Celcius. This is 0 if the GPU is currently powered off.
    pub temp_gpu: Celcius,

    /// The RPM of the left and right fans, respectively.
    pub fan_rpm: (u16, u16),

    /// The minimum fan speed permitted by the server, expressed as a decimal percentage.
    pub fan_speed_min: Percent,

    /// The fixed fan speed, expressed as a decimal percentage (i.e. `1.0` = 100%).
    /// If the fan state is `FanState::Fixed(u)` then `u == fan_speed_fixed.unwrap()`.
    /// This is duplicated outside of `fan_state` since it might be useful to know what the
    /// fixed fan speed was set to even if you're not using the fixed fan mode.
    /// This is `None` if the fan speed is set to an invalid value.
    pub fan_speed_fixed: Option<Percent>,

    /// The current fan mode. This is `None` if the fan is in an invalid state.
    pub fan_state: Option<FanState>,
}

/// Represents any error which occurred while submitting a request to the server,
/// retrieving the server's response, or conained in the server's response.
#[derive(Debug)]
pub enum ExchangeError {
    /// An error occurred while submitting a request.
    RequestError(EncodeError),
    /// An error occurred while reading the response.
    ResponseError(DecodeError),
    /// The server encountered an internal error preventing it from doing its job.
    InternalError,
    /// The server claims the client's request was malformed.
    MalformedRequest,
}

/// Represents a client connection to the a15kb server.
pub struct Connection {
    stream: net::UnixStream,
}
impl Connection {
    /// Creates a new connection to the server at `/var/a15kb/{socket_name}`.
    /// If you're unsure which socket name to use, try [`SOCKET_NAME`].
    pub fn new(socket_name: &str) -> Result<Self, std::io::Error> {
        let mut path = PathBuf::from(SOCKET_DIR);
        path.push(socket_name);
        net::UnixStream::connect(path).map(|stream| Self { stream })
    }

    /// Requests thermal information from the server. This is a blocking call.
    pub fn thermal_info(&mut self) -> ExchangeResult<ThermalInfo> {
        self.encode(Request::GetThermalInfo {})?;
        self.decode()
    }

    /// Requests to set the hardware fan state. This is a blocking call.
    pub fn set_fan_state(&mut self, fan_state: FanState) -> ExchangeResult<FanChangeResponse> {
        self.encode(Request::SetFanState(fan_state))?;
        self.decode()
    }

    /// Submits a raw [`Request`] to the socket. This is a blocking call.
    fn encode(&mut self, rq: Request) -> ExchangeResult<()> {
        bincode::encode_into_std_write(rq, &mut self.stream, BINCODE_CONFIG)?;
        Ok(())
    }
    /// Attempts to decode a value of type `T` from the socket. This is a blocking call.
    fn decode<T: Decode>(&mut self) -> ExchangeResult<T> {
        let header = bincode::decode_from_std_read(&mut self.stream, BINCODE_CONFIG)?;
        match header {
            ResponseHeader::Success => (),
            ResponseHeader::InternalError => return Err(ExchangeError::InternalError),
            ResponseHeader::MalformedRequest => return Err(ExchangeError::MalformedRequest),
        }
        let payload = bincode::decode_from_std_read(&mut self.stream, BINCODE_CONFIG)?;
        Ok(payload)
    }
}

pub type Celcius = u8;

/// A newtype wrapper around an `f32` which ensures the wrapped value is in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Decode, Encode)]
pub struct Percent(f32);

impl Percent {
    /// Creates a percent if the given value is in `0.0..=1.0`.
    pub fn new(value: f32) -> Option<Self> {
        (0.0..=1.0).contains(&value).then_some(Self(value))
    }
    /// Returns the wrapped float.
    pub const fn as_f32(self) -> f32 {
        self.0
    }
    /// Returns the averages of two percents.
    pub fn avg(a: Percent, b: Percent) -> Percent {
        Self(0.5 * (a.0 + b.0))
    }
}

impl Display for Percent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.0 * 100.0).fmt(f)?;
        f.write_str("%")
    }
}
impl From<Percent> for f32 {
    fn from(value: Percent) -> Self {
        value.as_f32()
    }
}

/// An error thrown when converting an `f32` to a [`Percent`].
#[derive(Debug)]
pub struct FromPercentError;

impl TryFrom<f32> for Percent {
    type Error = FromPercentError;
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(FromPercentError {})
    }
}

/// A request that the client sends to the server.
#[derive(Decode, Encode)]
enum Request {
    /// Retrieves thermal information. Success type: [`ThermalInfo`]
    GetThermalInfo,
    /// Sets the hardware fan state. Success type: [`FanChangeResponse`]
    SetFanState(FanState),
}

/// The header of the server's response. This indicates what happened and what (if any) payload data follows.
#[derive(Debug, Decode, Encode)]
enum ResponseHeader {
    /// The operation was "successful". The requested data follows this.
    Success,
    /// A serious internal error occurred in the server. No data follows this.
    InternalError,
    /// The client allegedly submitted a malformed request. No data follows this.
    MalformedRequest,
}

/// The bincode configuration used by both the client and the server.
pub(crate) const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

/// Convienence alias.
type ExchangeResult<T> = Result<T, ExchangeError>;

impl From<EncodeError> for ExchangeError {
    fn from(err: EncodeError) -> Self {
        Self::RequestError(err)
    }
}
impl From<DecodeError> for ExchangeError {
    fn from(err: DecodeError) -> Self {
        Self::ResponseError(err)
    }
}
impl Display for ExchangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestError(err) => {
                write!(f, "couldn't submit request to a15kb server: {}", err)
            }
            Self::ResponseError(err) => {
                write!(f, "couldn't retrieve response from a15kb server: {}", err)
            }
            Self::InternalError => {
                write!(f, "a15kb server had internal error")
            }
            Self::MalformedRequest => {
                write!(f, "a15kb server didn't understand our request")
            }
        }
    }
}
impl Error for ExchangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestError(err) => Some(err),
            Self::ResponseError(err) => Some(err),
            Self::InternalError => None,
            Self::MalformedRequest => None,
        }
    }
}
