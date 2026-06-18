use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::{Child, Command};

pub struct Amp2D {
    process: Child,
    url: String,
}

impl Amp2D {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn new<S: AsRef<OsStr>>(exe: S) -> Amp2D {
        let addr = TcpListener::bind(("0.0.0.0", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();

        let process = Command::new(&exe)
            .args([&addr])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();

        let url = format!("http://{addr}");

        Amp2D { process, url }
    }
}

impl Drop for Amp2D {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
