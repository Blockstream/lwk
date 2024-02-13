#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Instances of [testcontainers](https://docs.rs/testcontainers/latest/testcontainers/):
//!
//! * [`PinServer`] [Pin Server](https://github.com/Blockstream/blind_pin_server)
//! * [`JadeEmulator`] [Jade emulator](https://github.com/Blockstream/Jade/)
//!

mod jade;
mod ledger;
mod pin_server;

pub use jade::{JadeEmulator, EMULATOR_PORT};
pub use ledger::{LedgerEmulator, LEDGER_EMULATOR_PORT};
pub use pin_server::{PinServer, PIN_SERVER_PORT};

pub use testcontainers;

// pub fn print_docker_logs_and_panic(id: &str) -> ! {
//     let output = std::process::Command::new("docker").arg("logs").arg(id).output().unwrap();
//     let s = String::from_utf8(output.stdout).unwrap();
//     println!("{s}");
//     panic!("print docker logs and panic");
// }
