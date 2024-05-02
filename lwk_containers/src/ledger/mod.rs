use std::collections::HashMap;
use std::env;

use testcontainers::{core::WaitFor, Image, ImageArgs};

pub const LEDGER_EMULATOR_PORT: u16 = 9999;

#[derive(Debug, Default)]
pub struct LedgerEmulator {
    volumes: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct SpeculosArgs;

impl ImageArgs for SpeculosArgs {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        let args = vec![
            "apps/app.elf".to_string(),
            "-m".to_string(),
            "nanos".to_string(),
            "--display".to_string(),
            "headless".to_string(),
            "--automation".to_string(),
            "file:apps/speculos-automation.json".to_string(),
        ];
        Box::new(args.into_iter())
    }
}

impl LedgerEmulator {
    pub fn new() -> Result<Self, std::io::Error> {
        // speculos.py needs the elf file
        // in speculos doc they add a volume in this way,
        // so we're doing the same here
        let mut volumes = HashMap::new();
        // FIXME: do something cleaner
        let mut cur = std::env::current_dir().expect("TODO");
        cur.push("../lwk_containers/src/ledger/apps");
        let cur_s = cur.into_os_string().into_string().expect("TODO");
        volumes.insert(cur_s, "/speculos/apps".to_string());
        Ok(LedgerEmulator { volumes })
    }
}

impl Image for LedgerEmulator {
    type Args = SpeculosArgs;

    fn name(&self) -> String {
        env::var("LEDGER_EMULATOR_IMAGE_NAME").unwrap_or("ghcr.io/ledgerhq/speculos".into())
    }

    fn tag(&self) -> String {
        env::var("LEDGER_EMULATOR_IMAGE_VERSION").unwrap_or("sha-b09b240".into())
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdErrMessage {
            message: "Seed initialized from environment".into(),
        }]
    }

    fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.volumes.iter())
    }
}
