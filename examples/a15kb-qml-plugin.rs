use a15kb::{Client, FanMode, Percent};
use cstr::cstr;
use qmetaobject::prelude::*;
use qmetaobject::{qml_register_singleton_type, QSingletonInit};
use std::ffi::CStr;

#[rustfmt::skip]
#[derive(Default, QObject)]
struct QA15KBController {
    base: qt_base_class!(trait QObject),
    error: qt_property!(QString; NOTIFY errored),
    errored: qt_signal!(),
    set_fans_quiet: qt_method!(
        fn set_fans_quiet(&mut self) {todo!()}
    ),
    set_fans_normal: qt_method!(
        fn set_fans_normal(&mut self) {todo!()}
    ),
    set_fans_aggressive: qt_method!(
        fn set_fans_aggressive(&mut self) {todo!()}
    ),
    set_fans_fixed: qt_method!(
        fn set_fans_fixed(&mut self, pcnt: f64) {
            todo!()
        }
    ),
    client: Option<Client>,
}
impl QSingletonInit for QA15KBController {
    fn init(&mut self) {
        match Client::new() {
            Ok(client) => self.client = Some(client),
            Err(err) => self.set_error(err),
        }
    }
}
impl QA15KBController {
    fn set_error(&mut self, err: impl ToString) {
        let err = err.to_string();
        eprintln!("ERROR: {err}");
        self.error = QString::from(err);
        self.errored();
    }
    fn set_fan_mode(&mut self, fan_mode: FanMode) {
        if let Some(client) = self.client.as_mut() {
            if let Err(err) = client.set_fan_mode(fan_mode) {
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
        qml_register_singleton_type::<QA15KBController>(uri, 1, 0, cstr!("Controller"));
    }
}
