use interprocess::local_socket;
use std::{sync::{Arc, Mutex}, io::{Write, BufReader, BufRead}};

macro_rules! fail {
    ($($tok:tt)*) => {
	    {
	       eprintln!($($tok)*);
	       std::process::exit(1)
	    } 
    };
}
pub fn main() {
	let controller = match a15kb::Controller::new() {
		Ok(c) => c,
		Err(e) => fail!("{e}\nIs the server running as root?"),
	};

	let controller = Arc::new(Mutex::new(controller));

	use local_socket::LocalSocketListener as LSL;
	let listener = match LSL::bind(a15kb::SERVER_NAME) {
		Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
			fail!("server already running (?)");
		}
		Err(e) => fail!("couldn't open socket: {e}"),
		Ok(l) => l
	};

	eprintln!("[info] server started at {}", a15kb::SERVER_NAME);
	
	let handle_error = |conn| {
		match conn {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("[warning] connection failed: {e}");
                None
            }
        }
	};

	for stream in listener.incoming().filter_map(handle_error) {
		eprintln!("[info] client connected");
		let controller = Arc::clone(&controller);
		std::thread::spawn(move || handle_client(controller, stream));
	}
}

fn handle_command(
	controller: &Arc<Mutex<a15kb::Controller>>, 
	stream: &mut BufReader<local_socket::LocalSocketStream>,
	should_terminate: &mut bool
) -> a15kb::Response {
	let mut buffer = Vec::with_capacity(128);
	match stream.read_until(b'\n', &mut buffer) {
		Ok(0) | Err(_) => {
			// an error occurred and the stream needs to be closed.
			*should_terminate = true;
			return a15kb::Response::Failure("stream closed".to_owned());
		}
		Ok(_) => (),
	}
	let command: a15kb::Command = match serde_json::from_slice(&buffer) {
		Ok(c) => c,
		Err(_) => return a15kb::Response::Skipped
	};
	let mut ctrl = controller.lock().unwrap();
	match command {
		a15kb::Command::SetFanMode(fm) => match ctrl.set_fan_mode(fm) {
			Ok(()) => a15kb::Response::GenericSuccess,
			Err(e) => a15kb::Response::Failure(e.to_string())
		}
		a15kb::Command::GetFanMode => match ctrl.get_fan_mode() {
			Ok(fm) => a15kb::Response::FanMode(fm),
			Err(e) => a15kb::Response::Failure(e.to_string())
		}
		a15kb::Command::GetFanRpm => match ctrl.get_fan_rpm() {
			Ok(rpm) => a15kb::Response::FanRpm(rpm),
			Err(e) => a15kb::Response::Failure(e.to_string())
		}
		a15kb::Command::Close => {
			*should_terminate = true;
			a15kb::Response::GenericSuccess
		}
	}
}
fn handle_client(
	controller: Arc<Mutex<a15kb::Controller>>, 
	stream: local_socket::LocalSocketStream
) {
	let mut stream = BufReader::new(stream);
	let mut should_terminate = false;
	while !should_terminate {
		let response = handle_command(
			&controller, 
			&mut stream, 
			&mut should_terminate
		);
		let mut output = serde_json::to_vec(&response).unwrap();
		output.push(b'\n');
		let _ = stream.get_mut().write_all(&output);
		let _ = stream.get_mut().flush();
	}
	println!("[info] client disconnected")
}