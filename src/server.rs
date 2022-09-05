use super::*;
use anyhow::{ensure, Context};
use dbus::blocking::Connection;
use dbus_crossroads::Crossroads;
use std::fs;
use std::io;
use std::io::{Read, Seek, Write};
use std::process::Command;

/// An error which occurred at the level of the embedded controller. This is
/// opaque, which is fine, since there's nothing you can really *do* about an
/// EC error (at least from userspace)
#[derive(Debug)]
struct EcError;
impl std::fmt::Display for EcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("error communicating with embedded controller")
    }
}
impl std::error::Error for EcError {}
impl From<EcError> for dbus::MethodErr {
    fn from(err: EcError) -> Self {
        dbus::MethodErr::failed(&err)
    }
}

/// Just eprintln but with locked stderr (so threads don't trample on each others' messages)
macro_rules! log {
     ($($tok:tt)*) => {
        {
            let stderr = ::std::io::stderr();
            let mut locked = stderr.lock();
            let _ = write!(&mut locked, $($tok)*);
        }
     };
}

/// Convienence macro that logs the error before returning EcError
macro_rules! ec_error {
    ($($tok:tt)*) => {
        {
            let id = ::std::thread::current().id();
            let msg = format!($($tok)*);
               log!("[warn]({:?}){}", id, msg);
               Err(EcError {})
        }
    }
}

#[allow(clippy::type_complexity)]
mod server_generated {
    include! { concat!(env!("OUT_DIR"), "/server_generated.rs") }
}

/// The configuration for the a15kb server.
#[derive(Debug, Default)]
pub struct ServerCfg {
    /// Whether to replace the existing service, if one exists.
    pub replace: bool,
}

/// Runs the a15kb server with the configuration given by `cfg`.
pub fn run_server(cfg: &ServerCfg) -> Result<(), anyhow::Error> {
    // Set up our controller
    let controller = Controller::new()?;

    // Connect to the system bus & grab the name
    // If we can't grab it, just error out, don't stall in the queue
    let cxn = Connection::new_system().context("couldn't connect to system bus")?;
    cxn.request_name(BUS_NAME, true, cfg.replace, true)
        .context("couldn't obtain bus name")?;

    // Set up our D-Bus object
    let mut cr = Crossroads::new();
    let token = server_generated::register_com_offbyond_a15kb_controller1(&mut cr);
    cr.insert("/com/offbyond/a15kb/Controller1", &[token], controller);

    // Let's go!
    eprintln!("[info] server started");
    cr.serve(&cxn)?;
    eprintln!("[info] server stopped");
    Ok(())
}

/// A D-Bus compatible, high-level wrapper around the raw embedded controller
struct Controller {
    ec: Ec,
}
impl Controller {
    pub fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            ec: Ec::new().context("error setting up embedded controller")?,
        })
    }
}
impl server_generated::ComOffbyondA15kbController1 for Controller {
    fn get_thermal_info(&mut self) -> Result<(u8, u8, (u16, u16), u8, f64), dbus::MethodErr> {
        let temp_cpu = self.ec.temp_cpu()?;
        let temp_gpu = self.ec.temp_gpu()?;
        let fan_rpm = self.ec.fan_rpm()?;
        let fan_state = match self.ec.fan_modes()? {
            // (quiet, gaming, fixed)
            // TODO: better way to keep this in check with FanMode?
            (false, false, false) => 0,
            (true, false, false) => 1,
            (false, true, false) => 2,
            (true, true, false) => u8::MAX, // quiet AND gaming?
            (_, _, true) => 3,
        };
        let fixed_fan_speed = {
            // TODO: Maybe expose each fan's speed individually?
            let (hw0, hw1) = self.ec.fan_fixed_hw_speeds()?;
            let fl0 = (hw0 as f64) / (HW_MAX_FAN_SPEED as f64);
            let fl1 = (hw1 as f64) / (HW_MAX_FAN_SPEED as f64);
            0.5 * (fl0 + fl1)
        };
        Ok((temp_cpu, temp_gpu, fan_rpm, fan_state, fixed_fan_speed))
    }
    fn set_fan_mode(&mut self, fan_mode: u8) -> Result<(), dbus::MethodErr> {
        let settings = match FanMode::from_discriminant(fan_mode) {
            Some(FanMode::Quiet) => (true, false, false),
            Some(FanMode::Normal) => (false, false, false),
            Some(FanMode::Gaming) => (false, true, false),
            Some(FanMode::Fixed) => (false, false, true),
            None => return Err(dbus::MethodErr::invalid_arg(&fan_mode)),
        };
        self.ec.set_fan_modes(settings)?;
        Ok(())
    }
    fn set_fixed_fan_speed(&mut self, fixed_fan_speed: f64) -> Result<(), dbus::MethodErr> {
        if !(FAN_FIXED_SPEED_MIN..=FAN_FIXED_SPEED_MAX).contains(&fixed_fan_speed) {
            return Err(dbus::MethodErr::invalid_arg(&fixed_fan_speed));
        }
        let fhw_speed = fixed_fan_speed * (HW_MAX_FAN_SPEED as f64);
        let hw_speed = fhw_speed as u8;
        self.ec.set_fan_fixed_hw_speeds((hw_speed, hw_speed))?;
        Ok(())
    }
    fn allowed_fan_speeds(&self) -> Result<(f64, f64), dbus::MethodErr> {
        Ok((FAN_FIXED_SPEED_MIN, FAN_FIXED_SPEED_MAX))
    }
}

/// The maximum integer speed of the fan. [Source.][source]
///
/// [source]: https://github.com/tangalbert919/p37-ec-aero-15/blob/master/Aero%2015%20Fan%20Control%20Registers.md#custom-fan-mode-auto-maximum
const HW_MAX_FAN_SPEED: u8 = 229;

/// The minimum allowable fixed fan speed
const FAN_FIXED_SPEED_MIN: f64 = 0.3;

/// The maximum allowable fixed fan speed
const FAN_FIXED_SPEED_MAX: f64 = 1.0;

/// Offsets (and possibly bit indices) of EC registers.
mod offs {
    /// Byte. The CPU temperature, in degrees celcius.
    pub const TEMP_CPU: u64 = 0x60;
    /// Byte. The dGPU temperature, in degrees celcius.
    /// This will report as 0 if the dGPU is turned off.
    pub const TEMP_GPU: u64 = 0x61;

    /// Bit. Set iff the fans are in quiet mode.
    pub const FAN_QUIET: (u64, u8) = (0x08, 6);
    /// Bit. Set iff the fans are in gaming ("aggressive") mode.
    pub const FAN_GAMING: (u64, u8) = (0x0C, 4);
    /// Bit. Set iff the fans are in fixed-speed mode.
    pub const FAN_FIXED: (u64, u8) = (0x06, 4);

    /// Byte. The fixed speed of the left fan (0 to [`HW_MAX_FAN_SPEED`] range)
    pub const FAN_FIXED_HW_SPEED_0: u64 = 0xB0;
    /// Byte. The fixed speed of the right fan (0 to [`HW_MAX_FAN_SPEED`] range)
    pub const FAN_FIXED_HW_SPEED_1: u64 = 0xB1;

    /// Big-endian DWORD. The left fan's RPM.
    pub const FAN_RPM_0: u64 = 0xFC;
    /// Big-endian DWORD. The right fan's RPM.
    pub const FAN_RPM_1: u64 = 0xFE;
}

/// Convienence type.
type EcResult<T> = Result<T, EcError>;

/// A wrapper around the embedded controller.
struct Ec {
    /// The embedded controller's memory, represented as a file.
    inner: fs::File,
}

impl Ec {
    /// Initializes a new controller instance.
    /// This uses `modprobe` to load `ec_sys` if it's not already loaded.
    /// This will fail if the system doesn't report itself to be "AERO 15 KB".
    fn new() -> Result<Self, anyhow::Error> {
        // Before we do anything else, make sure we're actually running on an Aero 15 KB.
        //
        // This seems a bit silly -- why would you install this if you're not running a supported
        // computer? -- but I'm actually developing this on a persistent USB install, which I could
        // theoretically try to run on another computer in the future.
        //
        // If you're have a different Aero model and want to run this anyways, you can disable the
        // safety check. Caveat emptor.
        #[cfg(all())]
        {
            let product_name = fs::read_to_string("/sys/class/dmi/id/product_name")
                .context("couldn't retrieve product name")?;
            ensure!(
                product_name == "AERO 15 KB\n",
                "unsupported hardware ({product_name})"
            );
        }

        // Load ec_sys kernel module so we can directly access the embedded controller.
        // I've heard rumors that ec_sys should be avoided, but never any explanation...
        let status = Command::new("modprobe")
            .arg("ec_sys")
            .arg("write_support=1")
            .status()
            .context("couldn't load ec_sys kernel module")?;
        ensure!(status.success(), "couldn't load ec_sys kernel module");

        // Open handle to embedded controller
        let inner = fs::File::options()
            .read(true)
            .write(true)
            .open("/sys/kernel/debug/ec/ec0/io")
            .context("couldn't access embedded controller")?;

        Ok(Self { inner })
    }

    /// Sets the file cursor to `offset` bytes from the start of the embedded controller data.
    ///
    /// # Safety
    /// This is *probably* safe, even on invalid hardware. Still, treat it as if it could brick your computer.
    unsafe fn set_offset(&mut self, offset: u64) -> EcResult<()> {
        match self.inner.seek(io::SeekFrom::Start(offset)) {
            Ok(pos) if pos == offset => Ok(()),
            Ok(_) => ec_error!("failed to access EC: seek error"),
            Err(err) => ec_error!("failed to access EC: {}", err),
        }
    }
    /// Fill up `buffer` by reading bytes from the given offset in the embedded controller.
    ///
    /// # Safety
    /// This is *probably* safe, even on invalid hardware. Still, treat it as if it could brick your computer.
    unsafe fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> EcResult<()> {
        self.set_offset(offset)?;
        match self.inner.read(buffer) {
            Ok(num_read) if num_read == buffer.len() => Ok(()),
            Ok(_) => ec_error!("failed to read EC: not enough read"),
            Err(err) => ec_error!("failed to read EC: {}", err),
        }
    }

    /// Read the byte at the given offset of the embedded controller.
    ///
    /// # Safety
    /// Same as [`read_bytes`].
    unsafe fn read_byte(&mut self, offset: u64) -> EcResult<u8> {
        let mut byte = 0u8;
        self.read_bytes(offset, std::slice::from_mut(&mut byte))?;
        Ok(byte)
    }

    /// Read the selected bit from the byte at the given offset of the embedded controller.
    ///
    /// # Panics
    /// Panics if the bit index is out of range (i.e. not in 0..=7)
    ///
    /// # Safety
    /// Same as [`read_bytes`].
    unsafe fn read_bit(&mut self, (offset, bit): (u64, u8)) -> EcResult<bool> {
        let byte = self.read_byte(offset)?;
        let shifted = byte.checked_shr(bit.into()).expect("invalid bit index");
        let extracted = shifted & 1;
        Ok(extracted != 0)
    }

    /// Returns the CPU temperature in degrees Celcius.
    fn temp_cpu(&mut self) -> EcResult<u8> {
        unsafe { self.read_byte(offs::TEMP_CPU) }
    }

    /// Returns the GPU temperature in degrees Celcius. This will return `0` if the GPU is powered off.
    fn temp_gpu(&mut self) -> EcResult<u8> {
        unsafe { self.read_byte(offs::TEMP_GPU) }
    }

    /// Returns the RPMs of the left and right fans, respectively.
    fn fan_rpm(&mut self) -> EcResult<(u16, u16)> {
        let (mut rpm0, mut rpm1) = ([0u8, 0u8], [0u8, 0u8]);
        unsafe {
            self.read_bytes(offs::FAN_RPM_0, &mut rpm0)?;
            self.read_bytes(offs::FAN_RPM_1, &mut rpm1)?;
        }
        Ok((u16::from_be_bytes(rpm0), u16::from_be_bytes(rpm1)))
    }

    /// Returns `(quiet, gaming, fixed)` where each bool represents whether
    /// that fan mode is set.
    ///
    /// Only one of the fan modes *should* be set, but it's possible that
    /// some other software (or firmware!) snuck behind our back and threw
    /// the fans into an invalid state.
    fn fan_modes(&mut self) -> EcResult<(bool, bool, bool)> {
        let quiet = unsafe { self.read_bit(offs::FAN_QUIET)? };
        let gaming = unsafe { self.read_bit(offs::FAN_GAMING)? };
        let fixed = unsafe { self.read_bit(offs::FAN_FIXED)? };
        Ok((quiet, gaming, fixed))
    }

    /// Returns the fixed hardware speed of the left and right fans,
    /// respectively. This works even when the fan isn't in fixed-speed
    /// mode.
    fn fan_fixed_hw_speeds(&mut self) -> EcResult<(u8, u8)> {
        let fan0 = unsafe { self.read_byte(offs::FAN_FIXED_HW_SPEED_0)? };
        let fan1 = unsafe { self.read_byte(offs::FAN_FIXED_HW_SPEED_1)? };
        Ok((fan0, fan1))
    }

    /// Write the contents of `buffer` to the given offset in the embedded controller.
    ///
    /// # Safety
    /// This could brick your computer.
    unsafe fn write_bytes(&mut self, offset: u64, buffer: &[u8]) -> EcResult<()> {
        self.set_offset(offset)?;
        match self.inner.write(buffer) {
            Ok(num_read) if num_read == buffer.len() => Ok(()),
            Ok(_) => ec_error!("failed to write EC: not enough written"),
            Err(err) => ec_error!("failed to write EC: {}", err),
        }
    }

    /// Write the contents of `buffer` to the given offset in the embedded controller.
    ///
    /// # Safety
    /// Same as [`write_bytes`].
    unsafe fn write_byte(&mut self, offset: u64, byte: u8) -> EcResult<()> {
        self.write_bytes(offset, std::slice::from_ref(&byte))
    }

    /// Write to the selected bit of the byte at the given offset of the embedded controller.
    ///
    /// # Panics
    /// Panics if the bit index is out of range (i.e. not in 0..=7)
    ///
    /// # Safety
    /// Same as [`write_bytes`].
    unsafe fn write_bit(&mut self, (offset, bit): (u64, u8), val: bool) -> EcResult<()> {
        let byte = self.read_byte(offset)?;
        let shifted = 1u8.checked_shl(bit.into()).expect("invalid bit index");
        let changed = if val { byte | shifted } else { byte & !shifted };
        self.write_byte(offset, changed)
    }

    /// Sets the computer's fan modes.
    ///
    /// # Panics
    /// Panics if `quiet && gaming`, since I haven't tested that combo yet and I'm afraid to do so.
    /// AFAIK there's no reason to want to set that anyways.
    fn set_fan_modes(&mut self, (quiet, gaming, fixed): (bool, bool, bool)) -> EcResult<()> {
        assert!(!(quiet && gaming));
        unsafe {
            self.write_bit(offs::FAN_QUIET, quiet)?;
            self.write_bit(offs::FAN_GAMING, gaming)?;
            self.write_bit(offs::FAN_FIXED, fixed)
        }
    }

    /// Sets the fixed fan hardware speeds.
    ///
    /// # Panics
    /// Panics if either speed is greater than [`HW_MAX_FAN_SPEED`].
    fn set_fan_fixed_hw_speeds(&mut self, (fan0, fan1): (u8, u8)) -> EcResult<()> {
        assert!(fan0 <= HW_MAX_FAN_SPEED);
        assert!(fan1 <= HW_MAX_FAN_SPEED);
        unsafe {
            self.write_byte(offs::FAN_FIXED_HW_SPEED_0, fan0)?;
            self.write_byte(offs::FAN_FIXED_HW_SPEED_1, fan1)
        }
    }
}
