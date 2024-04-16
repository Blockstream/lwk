use std::env;

use testcontainers::{core::WaitFor, Image};

pub const EMULATOR_PORT: u16 = 30_121;

#[derive(Debug, Default)]
pub struct JadeEmulator;

impl Image for JadeEmulator {
    type Args = ();

    fn name(&self) -> String {
        // TODO Change with blockstream official jade emulator
        env::var("JADE_EMULATOR_IMAGE_NAME").unwrap_or("xenoky/local-jade-emulator".into())
    }

    fn tag(&self) -> String {
        env::var("JADE_EMULATOR_IMAGE_VERSION").unwrap_or("1.0.27".into())
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdOutMessage {
            message: "char device redirected".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [EMULATOR_PORT].into()
    }
}
