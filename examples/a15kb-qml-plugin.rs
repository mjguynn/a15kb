use a15kb::{Connection, FanState, Percent, DEFAULT_SOCKET_NAME};
use cstr::cstr;
use qmetaobject::prelude::*;
use std::ffi::CStr;

#[rustfmt::skip]
#[derive(Default, QObject)]
struct QA15KBController {
    base: qt_base_class!(trait QObject),
    error: qt_property!(QString; NOTIFY errored),
    errored: qt_signal!(),
    set_fans_quiet: qt_method!(
        fn set_fans_quiet(&mut self) {self.set_fans(FanState::Quiet)}
    ),
    set_fans_normal: qt_method!(
        fn set_fans_normal(&mut self) {self.set_fans(FanState::Normal)}
    ),
    set_fans_aggressive: qt_method!(
        fn set_fans_aggressive(&mut self) {self.set_fans(FanState::Aggressive)}
    ),
    set_fans_fixed: qt_method!(
        fn set_fans_fixed(&mut self, pcnt: f32) {
            match Percent::try_from(pcnt) {
                Ok(pcnt) => self.set_fans(FanState::Fixed(pcnt)),
                Err(_) => self.set_error("invalid fan percentage")
            }
        }
    ),
    cxn: Option<Connection>,
}
impl QA15KBController {
    fn set_error(&mut self, err: impl ToString) {
        let err = err.to_string();
        eprintln!("ERROR: {err}");
        self.error = QString::from(err);
        self.errored();
    }
    fn lazy_init(&mut self) {
        if self.cxn.is_none() {
            match Connection::new(DEFAULT_SOCKET_NAME) {
                Ok(cxn) => self.cxn = Some(cxn),
                Err(err) => self.set_error(err),
            }
        }
    }
    fn set_fans(&mut self, fan_state: FanState) {
        self.lazy_init();
        if let Some(cxn) = self.cxn.as_mut() {
            if let Err(err) = cxn.set_fan_state(fan_state) {
                self.set_error(err);
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
        // TODO: use qml_register_singleton_type instead
        qml_register_type::<QA15KBController>(uri, 1, 0, cstr!("Controller"));
    }
}
