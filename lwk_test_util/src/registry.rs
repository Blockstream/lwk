use std::ffi::OsStr;
use std::process::{Child, Command};
// use std::time::Duration;

use crate::reserve_port;

pub struct RegistryD {
    process: Child,
    url: String,
    _dir: tempfile::TempDir,
}

impl RegistryD {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn new<S: AsRef<OsStr>>(exe: S, esplora_url: &str) -> RegistryD {
        let tmp = tempfile::tempdir().unwrap();
        let datadir = tmp.path().display().to_string();

        let addr = format!("0.0.0.0:{}", reserve_port());

        let process = Command::new(&exe)
            .args(["--addr", &addr])
            .args(["--db-path", &datadir])
            .args(["--esplora-url", esplora_url])
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let url = format!("http://{addr}");

        RegistryD {
            process,
            url,
            _dir: tmp,
        }
    }
}

impl Drop for RegistryD {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
