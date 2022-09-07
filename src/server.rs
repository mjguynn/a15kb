use super::*;
use anyhow::Context;
use dbus::blocking::Connection;
use dbus_crossroads::Crossroads;
use std::cell::RefCell;

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
    ec: RefCell<ec::Ec>,
}
impl Controller {
    /// Creates a new D-Bus controller if possible.
    pub fn new() -> Result<Self, anyhow::Error> {
        let ec = ec::Ec::new().context("error setting up embedded controller")?;
        Ok(Self {
            ec: RefCell::new(ec),
        })
    }
}
impl server_generated::ComOffbyondA15kbController1 for Controller {
    fn get_thermal_info(&mut self) -> Result<(u8, u8, (u16, u16)), dbus::MethodErr> {
        let ec = self.ec.get_mut();
        Ok((ec.temp_cpu()?, ec.temp_gpu()?, ec.fan_rpm()?))
    }
    fn fan_mode(&self) -> Result<u8, dbus::MethodErr> {
        let fan_mode = match self.ec.borrow_mut().fan_modes()? {
            // (quiet, gaming, fixed)
            (false, false, false) => FanMode::Normal.to_discriminant(),
            (true, false, false) => FanMode::Quiet.to_discriminant(),
            (false, true, false) => FanMode::Gaming.to_discriminant(),
            (true, true, false) => u8::MAX, // quiet AND gaming?
            (_, _, true) => FanMode::Fixed.to_discriminant(),
        };
        Ok(fan_mode)
    }
    fn set_fan_mode(&self, fan_mode: u8) -> Result<(), dbus::MethodErr> {
        let settings = match FanMode::from_discriminant(fan_mode) {
            Some(FanMode::Quiet) => (true, false, false),
            Some(FanMode::Normal) => (false, false, false),
            Some(FanMode::Gaming) => (false, true, false),
            Some(FanMode::Fixed) => (false, false, true),
            None => return Err(dbus::MethodErr::invalid_arg(&fan_mode)),
        };
        self.ec.borrow_mut().set_fan_modes(settings)?;
        Ok(())
    }
    fn fixed_fan_speed(&self) -> Result<f64, dbus::MethodErr> {
        let fixed_fan_speed = {
            // TODO: Maybe expose each fan's speed individually?
            let (hw0, hw1) = self.ec.borrow_mut().fan_fixed_hw_speeds()?;
            let fl0 = (hw0 as f64) / (ec::HW_MAX_FAN_SPEED as f64);
            let fl1 = (hw1 as f64) / (ec::HW_MAX_FAN_SPEED as f64);
            0.5 * (fl0 + fl1)
        };
        Ok(fixed_fan_speed)
    }
    fn set_fixed_fan_speed(&self, fixed_fan_speed: f64) -> Result<(), dbus::MethodErr> {
        if !(ec::FAN_FIXED_SPEED_MIN..=ec::FAN_FIXED_SPEED_MAX).contains(&fixed_fan_speed) {
            return Err(dbus::MethodErr::invalid_arg(&fixed_fan_speed));
        }
        let fhw_speed = fixed_fan_speed * (ec::HW_MAX_FAN_SPEED as f64);
        let hw_speed = fhw_speed as u8;
        self.ec
            .borrow_mut()
            .set_fan_fixed_hw_speeds((hw_speed, hw_speed))?;
        Ok(())
    }
    fn allowed_fixed_fan_speeds(&self) -> Result<(f64, f64), dbus::MethodErr> {
        Ok((ec::FAN_FIXED_SPEED_MIN, ec::FAN_FIXED_SPEED_MAX))
    }
}
