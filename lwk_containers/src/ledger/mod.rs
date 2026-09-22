use std::env;

use testcontainers::core::{ContainerPort, Image, Mount, WaitFor};

pub const LEDGER_EMULATOR_PORT: u16 = 9999;

#[derive(Debug)]
pub struct LedgerEmulator {
    name: String,
    tag: String,
    mounts: Vec<Mount>,
    ports: Vec<ContainerPort>,
}

impl LedgerEmulator {
    pub fn new() -> Result<Self, std::io::Error> {
        // speculos.py needs the elf file
        // in speculos doc they add a volume in this way,
        // so we're doing the same here
        // FIXME: do something cleaner
        let mut cur = std::env::current_dir().expect("TODO");
        cur.push("../lwk_containers/src/ledger/apps");
        let cur_s = cur.into_os_string().into_string().expect("TODO");

        let mounts = vec![Mount::bind_mount(cur_s, "/speculos/apps")];
        Ok(LedgerEmulator {
            name: env::var("LEDGER_EMULATOR_IMAGE_NAME")
                .unwrap_or("ghcr.io/ledgerhq/speculos".into()),
            tag: env::var("LEDGER_EMULATOR_IMAGE_VERSION").unwrap_or("sha-b09b240".into()),
            mounts,
            ports: vec![ContainerPort::from(LEDGER_EMULATOR_PORT)],
        })
    }
}

impl Image for LedgerEmulator {
    fn name(&self) -> &str {
        &self.name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr(
            "Seed initialized from environment",
        )]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &self.ports
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<std::borrow::Cow<'_, str>>> {
        vec![
            "apps/app.elf",
            "-m",
            "nanos",
            "--display",
            "headless",
            "--automation",
            "file:apps/speculos-automation.json",
        ]
    }

    fn mounts(&self) -> impl IntoIterator<Item = &Mount> {
        &self.mounts
    }
}
