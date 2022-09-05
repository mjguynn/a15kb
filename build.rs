fn main() {
    let make_opts = |server: bool| dbus_codegen::GenOpts {
        dbuscrate: "dbus".to_string(),
        crossroads: server,
        methodtype: None,
        skipprefix: None,
        serveraccess: dbus_codegen::ServerAccess::RefClosure,
        genericvariant: false,
        connectiontype: dbus_codegen::ConnectionType::Blocking,
        propnewtype: false,
        interfaces: None,
        command_line: "[this is a lie. look at build.rs]".to_string(),
    };
    const CONTROLLER: &str = "a15kb.Controller1.xml";
    println!("cargo:rerun-if-changed={CONTROLLER}");
    let xml = std::fs::read_to_string(CONTROLLER).expect("couldn't read interface");
    let out_dir = std::env::var_os("OUT_DIR").unwrap();

    let client_opts = make_opts(false);
    let client_code =
        dbus_codegen::generate(&xml, &client_opts).expect("couldn't generate client code");
    let client_path = std::path::Path::new(&out_dir).join("client_generated.rs");
    std::fs::write(&client_path, &client_code).unwrap();

    let server_opts = make_opts(true);
    let server_code =
        dbus_codegen::generate(&xml, &server_opts).expect("couldn't generate server code");
    let server_path = std::path::Path::new(&out_dir).join("server_generated.rs");
    std::fs::write(&server_path, &server_code).unwrap();
}
