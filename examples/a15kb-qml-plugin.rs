use a15kb::{Client, ThermalInfo};
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

    fixedFanSpeedMin: qt_property!(f64),
    fixedFanSpeedMax: qt_property!(f64),

    /// The last recorded thermal information.
    thermal_info: Mutex<a15kb::ThermalInfo>,

    /// Emitted whenever any thermal information is updated.
    thermalsChanged: qt_signal!(),

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
        let update_thermals = qmetaobject::queued_callback(move |()| {
            if let Some(obj) = qptr.as_ref() {
                if let Some(client) = obj.client.as_ref() {
                    let thermal_info = client.thermal_info().unwrap_or_default();
                    *obj.thermal_info.lock().unwrap() = thermal_info;
                    obj.thermalsChanged();
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
            update_thermals(());
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
