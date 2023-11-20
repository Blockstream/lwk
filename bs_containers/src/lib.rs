//! Instances of [testcontainers](https://docs.rs/testcontainers/latest/testcontainers/):
//!
//! * [`pin_server::PinServerEmulator`] [Pin Server](https://github.com/Blockstream/blind_pin_server)
//! * [`jade::JadeEmulator`] [Jade emulator](https://github.com/Blockstream/Jade/)
//!

use std::process::Command;

pub mod jade;
pub mod pin_server;

pub use testcontainers;

pub fn print_docker_logs_and_panic(id: &str) -> ! {
    let output = Command::new("docker").arg("logs").arg(id).output().unwrap();
    let s = String::from_utf8(output.stdout).unwrap();
    println!("{s}");
    panic!("print docker logs and panic");
}
