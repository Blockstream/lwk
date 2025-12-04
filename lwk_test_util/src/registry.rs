use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::{Child, Command};
// use std::time::Duration;

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

        // 0 means the OS choose a free port
        let addr = TcpListener::bind(("0.0.0.0", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();

        let process = Command::new(&exe)
            .args(["--addr", &addr])
            .args(["--db-path", &datadir])
            .args(["--esplora-url", &esplora_url])
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
