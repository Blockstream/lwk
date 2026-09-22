use std::env;

use testcontainers::core::{ContainerPort, Image, WaitFor};

pub const EMULATOR_PORT: u16 = 30_121;

#[derive(Debug)]
pub struct JadeEmulator {
    name: String,
    tag: String,
    ports: Vec<ContainerPort>,
}

impl Image for JadeEmulator {
    fn name(&self) -> &str {
        // TODO Change with blockstream official jade emulator
        &self.name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("char device redirected")]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &self.ports
    }
}

impl JadeEmulator {
    pub fn new() -> Self {
        Self {
            name: env::var("JADE_EMULATOR_IMAGE_NAME")
                .unwrap_or("xenoky/local-jade-emulator".into()),
            tag: env::var("JADE_EMULATOR_IMAGE_VERSION").unwrap_or("1.0.41-beta1".into()),
            ports: vec![ContainerPort::from(EMULATOR_PORT)],
        }
    }
}

impl Default for JadeEmulator {
    fn default() -> Self {
        Self::new()
    }
}
