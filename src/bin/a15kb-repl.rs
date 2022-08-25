use std::io::{Write, BufRead, BufReader};

use interprocess::local_socket;
use local_socket::LocalSocketStream as LSS;

fn submit_command(
	stream: &mut BufReader<LSS>,
	cmd: &a15kb::Command
) -> Result<a15kb::Response, serde_json::Error> {
	let mut serialized = serde_json::to_vec(cmd).unwrap();
	serialized.push(b'\n');
	stream.get_mut().write(&serialized).unwrap();
	stream.get_mut().flush().unwrap();
	let mut response = Vec::with_capacity(128);
	stream.read_until(b'\n', &mut response).unwrap();
	serde_json::from_slice(&response)
}

macro_rules! match_rsp {
	($matched:expr, $p:pat => $do:expr) => {
		match $matched {
			Ok($p) => $do,
			Ok(a15kb::Response::Failure(e)) => eprintln!("server failure: {e}"),
			Ok(r) => eprintln!("invalid server response {r:?}"),
			Err(e) => eprintln!("malformed server response: {e}"),
		}
	}
}
fn get_fan_rpm(stream: &mut BufReader<LSS>) {
	match_rsp!(
		submit_command(stream, &a15kb::Command::GetFanRpm),
		a15kb::Response::FanRpm(rpm) => println!("fan RPM: {} RPM, {} RPM", rpm.0, rpm.1)
	)
}
fn get_fan_mode(stream: &mut BufReader<LSS>) {
	match_rsp!(
		submit_command(stream, &a15kb::Command::GetFanMode),
		a15kb::Response::FanMode(fm) => println!("fan mode: {fm}")
	)
}
fn set_fan_mode(stream: &mut BufReader<LSS>, fm: a15kb::FanMode) {
	match_rsp!(
		submit_command(stream, &a15kb::Command::SetFanMode(fm)),
		a15kb::Response::GenericSuccess => println!("fan mode set")
	)
}
fn close(stream: &mut BufReader<LSS>) {
	match_rsp!(
		submit_command(stream, &a15kb::Command::Close),
		a15kb::Response::GenericSuccess => println!("connection successfully closed")
	)
}
pub fn main() {
	let stream = match LSS::connect(a15kb::SERVER_NAME) {
		Ok(conn) => conn,
		Err(e) => {
			eprintln!("Couldn't connect to server {} ({e})", a15kb::SERVER_NAME);
			std::process::exit(1)
		}
	};
	let mut stream = BufReader::new(stream);
	let sfm_prefix = "set fan mode: ";
	for line in std::io::stdin().lines() {
		let line = line.unwrap();
		let line = line.as_str();
		if line == "get fan rpm" {
			get_fan_rpm(&mut stream);
		} else if line == "get fan mode" {
			get_fan_mode(&mut stream);
		} else if line == "exit" {
			close(&mut stream);
			std::process::exit(0);
		} else if line.starts_with(sfm_prefix) {
			let rest = &line[sfm_prefix.len()..];
			let fixed_prefix = "fixed ";
			let fm = if rest == "normal" {
				a15kb::FanMode::Normal
			} else if rest == "quiet" {
				a15kb::FanMode::Quiet
			} else if rest == "gaming" {
				a15kb::FanMode::Gaming
			} else if rest.starts_with(fixed_prefix) {
				let pnctr = &rest[fixed_prefix.len()..];
				let pnct = pnctr.split_once('%').unwrap();
				assert!(pnct.1 == "");
				let val = pnct.0.parse::<f32>().unwrap() / 100.0;
				a15kb::FanMode::Custom(val)
			} else {
				panic!("unknown fan mode");
			};
			set_fan_mode(&mut stream, fm);
		} else {
			eprintln!("unknown command")
		}
		
	}
}