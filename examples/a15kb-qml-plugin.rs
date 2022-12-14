use a15kb::{Client, FanMode, Percent, ThermalInfo};
use cstr::cstr;
use qmetaobject::prelude::*;
use qmetaobject::{qml_register_singleton_type, QSingletonInit};
use std::ffi::CStr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

/// This thread timer design is taken from
/// https://github.com/woboq/qmetaobject-rs/blob/master/examples/qmlextensionplugins/src/lib.rs
#[derive(Default)]
struct ThreadState {
    should_terminate: Mutex<bool>,
    wakeup: Condvar,
}

#[rustfmt::skip]
#[allow(non_snake_case)]
#[derive(Default, QObject)]
struct QA15KBController {
    base: qt_base_class!(trait QObject),

    // Published thermal information.
    cpuTemp: qt_property!(u8; NOTIFY thermalsChanged READ temp_cpu),
    gpuTemp: qt_property!(u8; NOTIFY thermalsChanged READ temp_gpu),
    fanRpm0: qt_property!(u16; NOTIFY thermalsChanged READ fan_rpm_0),
    fanRpm1: qt_property!(u16; NOTIFY thermalsChanged READ fan_rpm_1),

    fixedFanSpeedMin: qt_property!(f64; NOTIFY constPopulated),
    fixedFanSpeedMax: qt_property!(f64; NOTIFY constPopulated),

    /// The last recorded thermal information.
    thermal_info: Mutex<ThermalInfo>,

    /// Emitted whenever any thermal information is updated.
    thermalsChanged: qt_signal!(),

    /// The last recorded fan state (mode + fixed fan speed)
    fan_state: Mutex<(FanMode, Percent)>,

    /// Emitted when the fan state changes.
    fanStateChanged: qt_signal!(fan_mode: u8, fixed_fan_speed: f64),

    /// Emitted when the singleton's constant attributes are populated.
    constPopulated: qt_signal!(),

    setFanMode: qt_method!(fn(&mut self, mode: u8)),
    setFixedFanSpeed: qt_method!(fn(&mut self, speed: f64)),
    
    client: Option<Client>,

    thread: Option<(thread::JoinHandle<()>, Arc<ThreadState>)>
}
impl QSingletonInit for QA15KBController {
    fn init(&mut self) {
        // Really, we should immediately panic if we can't create a client...
        // ...but it would be Very Bad™ to bring the desktop down with us!
        match Client::new() {
            Ok(client) => {
                if let Ok(range) = client.allowed_fixed_fan_speeds() {
                    self.fixedFanSpeedMin = range.start().as_f64();
                    self.fixedFanSpeedMax = range.end().as_f64();
                    self.constPopulated();
                }
                self.client = Some(client);
            }
            Err(err) => eprintln!("[error] couldn't initialize client: {err}"),
        }

        let thread_state = ThreadState {
            should_terminate: Mutex::new(false),
            wakeup: Condvar::new(),
        };

        let qptr = QPointer::from(&*self);
        let update = qmetaobject::queued_callback(move |()| {
            if let Some(obj) = qptr.as_ref() {
                if let Some(client) = obj.client.as_ref() {
                    // Update thermals
                    let thermal_info = client.thermal_info().unwrap_or_default();
                    *obj.thermal_info.lock().unwrap() = thermal_info;
                    obj.thermalsChanged();

                    // Potentially update fan state
                    let mut fan_state = obj.fan_state.lock().unwrap();
                    let new_fan_state = (
                        client.fan_mode().unwrap_or_default().unwrap_or_default(),
                        client.fixed_fan_speed().unwrap_or_default(),
                    );
                    if *fan_state != new_fan_state {
                        *fan_state = new_fan_state;
                        obj.fanStateChanged(
                            new_fan_state.0.to_discriminant(),
                            new_fan_state.1.as_f64(),
                        );
                    }
                }
            }
        });

        let thread_state = Arc::new(thread_state);
        let thread_state_send = Arc::clone(&thread_state);

        let handle = thread::spawn(move || loop {
            let lock = thread_state_send.should_terminate.lock().unwrap();
            if *lock {
                return;
            }
            let lock = thread_state_send
                .wakeup
                .wait_timeout(lock, Duration::from_millis(1000))
                .unwrap()
                .0;
            std::mem::drop(lock);
            update(());
        });
        self.thread = Some((handle, thread_state));
    }
}
impl Drop for QA15KBController {
    fn drop(&mut self) {
        // (We shouldn't panic from a Drop impl, so ignore any errors.)
        if let Some((handle, thread_state)) = self.thread.take() {
            // Proclaim that the thread must die, then forcibly wake it up
            // from its deep and dreamless slumber.
            if let Ok(mut lock) = thread_state.should_terminate.lock() {
                *lock = true;
            }
            thread_state.wakeup.notify_one();
            // Block on the thread just to make sure everything's working right.
            let _ = handle.join();
        }
    }
}
impl QA15KBController {
    fn thermals(&mut self) -> MutexGuard<'_, ThermalInfo> {
        self.thermal_info.lock().unwrap()
    }
    fn temp_cpu(&mut self) -> u8 {
        self.thermals().temp_cpu
    }
    fn temp_gpu(&mut self) -> u8 {
        self.thermals().temp_gpu
    }
    fn fan_rpm_0(&mut self) -> u16 {
        self.thermals().fan_rpm.0
    }
    fn fan_rpm_1(&mut self) -> u16 {
        self.thermals().fan_rpm.1
    }
    #[allow(non_snake_case)]
    fn setFixedFanSpeed(&mut self, speed: f64) {
        if let Ok(pcnt) = Percent::try_from(speed) {
            if let Some(client) = self.client.as_ref() {
                let _ = client.set_fixed_fan_speed(pcnt);
            }
        }
    }
    #[allow(non_snake_case)]
    fn setFanMode(&mut self, mode: u8) {
        if let Some(fan_mode) = FanMode::from_discriminant(mode) {
            if let Some(client) = self.client.as_ref() {
                let _ = client.set_fan_mode(fan_mode);
            }
        }
    }
}

#[derive(Default, QObject)]
struct QA15KBQmlPlugin {
    base: qt_base_class!(trait QQmlExtensionPlugin),
    plugin: qt_plugin!("org.qt-project.Qt.QQmlExtensionInterface/1.0"),
}

impl QQmlExtensionPlugin for QA15KBQmlPlugin {
    fn register_types(&mut self, uri: &CStr) {
        assert_eq!(uri, cstr!("com.offbyond.a15kb"));
        qml_register_singleton_type::<QA15KBController>(uri, 1, 0, cstr!("Controller"));
    }
}
