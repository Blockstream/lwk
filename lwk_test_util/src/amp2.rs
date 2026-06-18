use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

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

        let mut process = Command::new(&exe)
            .args([&addr])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();

        match process.try_wait() {
            Ok(Some(_)) | Err(_) => {
                let _ = process.kill();
                panic!("failed to start amp2_mock");
            }
            Ok(None) => {}
        }

        let url = format!("http://{addr}");

        let mut started = false;
        let check_url = format!("{url}/info/xpub");
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(r) = reqwest::blocking::get(&check_url) {
                if r.status().as_u16() == 200 {
                    started = true;
                    break;
                }
            }
        }
        assert!(started, "amp2_mock hasn't started after 5s");

        Amp2D { process, url }
    }
}

impl Drop for Amp2D {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
