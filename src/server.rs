use super::*;
use anyhow::{ensure, Context};
use std::fs;
use std::io;
use std::io::{Read, Seek, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{self, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

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
	       	Err(InternalError::EcError {})
	    }
	}
}

/// Runs the a15kb server.
pub fn run_server(socket_name: &Path) -> Result<(), anyhow::Error> {
    // Access the embedded controller and prepare it for multithreaded access.
    let ec = Arc::new(Mutex::new(Ec::new()?));

    // Create the socket directory if it doesn't already exist.
    if let Err(err) = fs::create_dir(SOCKET_DIR) {
        ensure!(
            err.kind() == io::ErrorKind::AlreadyExists,
            "couldn't create socket directory"
        );
    }

    // Make sure everyone has R/X permissions for that directory.
    // Only we (the server) should have write permissions.
    let rx = fs::Permissions::from_mode(0o755);
    fs::set_permissions(SOCKET_DIR, rx).context("couldn't set socket directory permissions")?;

    let mut path = PathBuf::from(SOCKET_DIR);
    path.push(socket_name);

    // Remove the socket file if it already exists.
    if let Err(err) = fs::remove_file(&path) {
        ensure!(
            err.kind() == io::ErrorKind::NotFound,
            "couldn't remove existing socket"
        );
    }

    let listener = net::UnixListener::bind(&path).context("couldn't bind socket")?;

    let rw = fs::Permissions::from_mode(0o766);
    fs::set_permissions(&path, rw).context("couldn't set socket file permissions")?;

    eprintln!("[info] server started");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let ec = Arc::clone(&ec);
                thread::spawn(move || handle_connection(stream, ec));
            }
            Err(err) => log!("[warn] client connection failed: {err}"),
        }
    }

    // This is actually unreachable.
    Ok(())
}

fn handle_connection(mut stream: UnixStream, ec: Arc<Mutex<Ec>>) {
    let id = thread::current().id();
    println!("[info]({id:?}) client connected");
    while !handle_request(&mut stream, &ec) {}
    println!("[info]({id:?}) client disconnected");
}

/// Handles the next request from the stream. Returns whether the server should terminate the connection.
fn handle_request(stream: &mut UnixStream, ec: &Mutex<Ec>) -> bool {
    let request = bincode::decode_from_std_read(stream, BINCODE_CONFIG);
    match request {
        Ok(Request::GetThermalInfo) => send_response(get_thermal_info(ec), stream),
        Ok(Request::SetFanState(fans)) => todo!(),
        // reached end of stream unexpectedly, terminate connection
        Err(bincode::error::DecodeError::UnexpectedEnd) => return true,
        Err(_) => {
            let _ = bincode::encode_into_std_write(
                ResponseHeader::MalformedRequest,
                stream,
                BINCODE_CONFIG,
            );
        }
    }
    return false;
}

/// Sends `response` with the appropriate headers. Discards errors.
fn send_response<T: Encode>(response: InternalResult<T>, stream: &mut UnixStream) {
    match response {
        Ok(val) => {
            let _ = bincode::encode_into_std_write(ResponseHeader::Success, stream, BINCODE_CONFIG);
            let _ = bincode::encode_into_std_write(val, stream, BINCODE_CONFIG);
        }
        Err(_) => {
            let _ = bincode::encode_into_std_write(
                ResponseHeader::InternalError,
                stream,
                BINCODE_CONFIG,
            );
        }
    }
}
fn get_thermal_info(ec: &Mutex<Ec>) -> InternalResult<ThermalInfo> {
    // It's unacceptable for this function to panic! The mutex will be poisoned and the `unwrap`
    // will fail, bringing down all other connections in a cascade of panics!
    let mut ec = ec.lock().unwrap();
    let temp_cpu = ec.temp_cpu()?;
    let temp_gpu = ec.temp_gpu()?;
    let fan_rpm = ec.fan_rpm()?;
    let fan_speed_min = Percent::try_from(FAN_FIXED_SPEED_MIN).unwrap();
    let fan_speed_fixed = {
        let (hw0, hw1) = ec.fan_fixed_hw_speeds()?;
        let fl0 = (hw0 as f32) / (HW_MAX_FAN_SPEED as f32);
        let fl1 = (hw1 as f32) / (HW_MAX_FAN_SPEED as f32);
        let cvt0 = Percent::try_from(fl0);
        let cvt1 = Percent::try_from(fl1);
        match (cvt0, cvt1) {
            (Ok(pcnt0), Ok(pcnt1)) => Some(Percent::avg(pcnt0, pcnt1)),
            _ => None,
        }
    };
    let fan_state = match ec.fan_modes()? {
        // (quiet, gaming, fixed)
        (false, false, false) => Some(FanState::Normal),
        (true, false, false) => Some(FanState::Quiet),
        (false, true, false) => Some(FanState::Aggressive),
        (true, true, false) => None, // quiet AND gaming?
        (_, _, true) => fan_speed_fixed.map(FanState::Fixed),
    };
    Ok(ThermalInfo {
        temp_cpu,
        temp_gpu,
        fan_rpm,
        fan_speed_min,
        fan_speed_fixed,
        fan_state,
    })
}

/// The maximum integer speed of the fan. [Source.][source]
///
/// [source]: https://github.com/tangalbert919/p37-ec-aero-15/blob/master/Aero%2015%20Fan%20Control%20Registers.md#custom-fan-mode-auto-maximum
const HW_MAX_FAN_SPEED: u8 = 229;

/// The minimum allowable fixed fan speed, as a percentage (but not stored as a percent because floats don't like CTFE)
const FAN_FIXED_SPEED_MIN: f32 = 0.3;

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

/// An error which occurred at the level of the embedded controller. This is
/// pretty opaque, which is fine, since there's nothing you can really *do*
/// about an EC error (at least from userspace)
enum InternalError {
    /// An error which occurred at the level of the embedded controller. This is
    /// pretty opaque, which is fine, since there's nothing you can really *do*
    /// about an EC error (at least from userspace)
    EcError,
}

/// Convienence type.
type InternalResult<T> = Result<T, InternalError>;

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

    /// Fill up `buffer` by reading bytes from the given offset in the embedded controller.
    ///
    /// # Safety
    /// This is *probably* safe, even on invalid hardware. Still, treat it as if it could brick your computer.
    unsafe fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> InternalResult<()> {
        match self.inner.seek(io::SeekFrom::Start(offset)) {
            Ok(pos) if pos == offset => (),
            Ok(_) => return ec_error!("failed to access EC: seek error"),
            Err(err) => return ec_error!("failed to access EC: {}", err),
        }
        match self.inner.read(buffer) {
            Ok(num_read) if num_read == buffer.len() => Ok(()),
            Ok(_) => ec_error!("failed to read EC: not enough read"),
            Err(err) => ec_error!("failed to read EC: {}", err),
        }
    }

    /// Read the byte at the given offset of the embedded controller.
    ///
    /// # Safety
    /// Same as `read_bytes`.
    unsafe fn read_byte(&mut self, offset: u64) -> InternalResult<u8> {
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
    /// Same as `read_bytes`.
    unsafe fn read_bit(&mut self, offset_and_bit: (u64, u8)) -> InternalResult<bool> {
        let (offset, bit) = offset_and_bit;
        let byte = self.read_byte(offset)?;
        let shifted = byte.checked_shr(bit.into()).expect("invalid bit index");
        let extracted = shifted & 1;
        Ok(extracted != 0)
    }

    /// Returns the CPU temperature in degrees Celcius.
    fn temp_cpu(&mut self) -> InternalResult<u8> {
        unsafe { self.read_byte(offs::TEMP_CPU) }
    }

    /// Returns the GPU temperature in degrees Celcius. This will return `0` if the GPU is powered off.
    fn temp_gpu(&mut self) -> InternalResult<u8> {
        unsafe { self.read_byte(offs::TEMP_GPU) }
    }

    /// Returns the RPMs of the left and right fans, respectively.
    fn fan_rpm(&mut self) -> InternalResult<(u16, u16)> {
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
    fn fan_modes(&mut self) -> InternalResult<(bool, bool, bool)> {
        let quiet = unsafe { self.read_bit(offs::FAN_QUIET)? };
        let gaming = unsafe { self.read_bit(offs::FAN_GAMING)? };
        let fixed = unsafe { self.read_bit(offs::FAN_FIXED)? };
        Ok((quiet, gaming, fixed))
    }

    /// Returns the fixed hardware speed of the left and right fans,
    /// respectively. This works even when the fan isn't in fixed-speed
    /// mode.
    fn fan_fixed_hw_speeds(&mut self) -> InternalResult<(u8, u8)> {
        let fan0 = unsafe { self.read_byte(offs::FAN_FIXED_HW_SPEED_0)? };
        let fan1 = unsafe { self.read_byte(offs::FAN_FIXED_HW_SPEED_1)? };
        Ok((fan0, fan1))
    }

    /*
    /// Write the contents of `buffer` to the given offset in the embedded controller.
    ///
    /// # Safety
    /// This could brick your computer.
    unsafe fn write_bytes(&mut self, offset: u64, buffer: &[u8]) -> Result<(), String> {
        if let Err(err) = self.inner.seek(io::SeekFrom::Start(offset)) {
            return nonfatal!("failed to access EC: {}", err);
        }
        let mut ex = || {
            self.ec_handle.seek(io::SeekFrom::Start(offset))?;
            let num_written = self.ec_handle.write(buffer)?;
            if num_written != buffer.len() {
                return Err(std::io::ErrorKind::Other.into());
            }
            Ok(())
        };
        ex().map_err(|source| EK::EcWriteError { offset, source }.into())
    }
    */

    /*
    fn write_bit(&mut self, offset: u64, bit: u8, val: bool) -> Result<()> {
        // TODO: You can have separate fixed speeds for each of the fans.
        // Right now, we read both speeds and average them. Maybe we should
        // expose each fan speed invidually?
        let mut fixed_speeds = [0u8, 0u8];
        self.read_bytes(0xB0, &mut fixed_speeds)?;

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
    }*/
}
