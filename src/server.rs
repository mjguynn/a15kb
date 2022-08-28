use super::*;
use anyhow::{bail, ensure, Context};
use std::fs;
use std::io;
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
         	use ::std::io::Write;
         	let stderr = ::std::io::stderr();
         	let mut locked = stderr.lock();
         	let _ = write!(&mut locked, $($tok)*);
         }
     };
}

/// Runs the a15kb server.
pub fn run_server(socket_name: &Path) -> Result<(), anyhow::Error> {
    // Before we do anything else, make sure we're actually running on an Aero 15 KB.
    //
    // This seems a bit silly -- why would you install this if you're not running a supported
    // computer? -- but I'm actually developing this on a persistent USB install, which I could
    // theoretically try to run on another computer in the future.
    //
    // If you're using another Aero model and want to run this, you can disable this check.
    // Caveat emptor.
    #[cfg(on)]
    {
        let product_name = fs::read_to_string("/sys/class/dmi/id/product_name")
            .context("couldn't retrieve product name")?;
        ensure!(
            product_name == "AERO 15 KB",
            "unsupported hardware ({product_name})"
        );
    }

    // Access the embedded controller and prepare it for multithreaded access.
    let controller = Controller::new()?;
    let controller = Arc::new(Mutex::new(controller));

    // Create the socket directory if it doesn't already exist.
    if let Err(err) = fs::create_dir(SOCKET_DIR) {
        ensure!(
            err.kind() == io::ErrorKind::AlreadyExists,
            "couldn't create socket directory"
        );
    }

    // Make sure everyone has R/W permissions for that directory.
    let rw = fs::Permissions::from_mode(0o666);
    fs::set_permissions(SOCKET_DIR, rw).context("couldn't set socket directory permissions")?;

    let mut path = PathBuf::from(SOCKET_DIR);
    path.push(socket_name);

    let listener = net::UnixListener::bind(path).context("couldn't bind socket")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let controller = Arc::clone(&controller);
                thread::spawn(move || handle_connection(stream, controller));
            }
            Err(err) => log!("[warn] client connection failed: {err}"),
        }
    }

    // This is actually unreachable.
    Ok(())
}

fn handle_connection(mut stream: UnixStream, controller: Arc<Mutex<Controller>>) {
    let id = thread::current().id();
    println!("[info] client connected (thread {id:?})");
    loop {
        handle_request(&mut stream, &controller);
    }
    println!("[info] client disconnected (thread {id:?})");
}
fn handle_request(stream: &mut UnixStream, controller: &Mutex<Controller>) {
    let request = bincode::decode_from_std_read(stream, BINCODE_CONFIG);
    match request {
        Ok(Request::GetThermalInfo) => todo!(),
        Ok(Request::SetFanState(fans)) => todo!(),
        Err(err) => todo!(),
    }
}

struct Controller(fs::File);

impl Controller {
    pub fn new() -> Result<Self, anyhow::Error> {
        // Load ec_sys kernel module so we can directly access the embedded controller.
        // I've heard rumors that ec_sys should be avoided, but never any explanation...
        let status = Command::new("modprobe")
            .arg("ec_sys")
            .arg("write_support=1")
            .status()
            .context("couldn't load ec_sys kernel module")?;
        ensure!(status.success(), "couldn't load ec_sys kernel module");

        // Open handle to embedded controller
        let controller = fs::File::options()
            .read(true)
            .write(true)
            .open("/sys/kernel/debug/ec/ec0/io")
            .context("couldn't access embedded controller")?;

        Ok(Self(controller))
    }
    /*
    fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        self.ec_handle
            .seek(io::SeekFrom::Start(offset))
            .map_err(|source| EK::EcAccessError { source })?;
        let num_read = self
            .ec_handle
            .read(buffer)
            .map_err(|source| EK::EcReadError { offset, source })?;
        if num_read != buffer.len() {
            Err(EK::EcReadError {
                offset,
                source: io::ErrorKind::Other.into(),
            }
            .into())
        } else {
            Ok(())
        }
    }
    fn write_bytes(&mut self, offset: u64, buffer: &[u8]) -> Result<()> {
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
    }*/
}
