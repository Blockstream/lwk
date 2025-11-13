use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

pub struct WaterfallsD {
    process: Child,
    waterfalls_url: String,
}

impl WaterfallsD {
    pub fn waterfalls_url(&self) -> &str {
        &self.waterfalls_url
    }

    pub fn new<S: AsRef<OsStr>>(
        exe: S,
        elements_url: &str,
        rpcuser: &str,
        rpcpassword: &str,
    ) -> WaterfallsD {
        // 0 means the OS choose a free port
        let addr = TcpListener::bind(("0.0.0.0", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        let user_pass = format!("{rpcuser}:{rpcpassword}");

        let args = vec![
            "--network",
            "elements-regtest",
            "--node-url",
            elements_url,
            "--rpc-user-password",
            &user_pass,
            "--listen",
            &addr,
        ];
        let waterfalls_url = format!("http://{addr}");

        let mut process = Command::new(&exe).args(args).spawn().unwrap();

        match process.try_wait() {
            Ok(Some(_)) | Err(_) => {
                // Process has exited or an error occurred, kill and retry
                let _ = process.kill();
                panic!("failed to start waterfalls");
            }
            Ok(None) => {} // Process is still running, proceed
        }

        // Wait for waterfalls to start
        let mut started = false;
        let url = format!("{}/blocks/tip/hash", waterfalls_url);
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(r) = reqwest::blocking::get(&url) {
                if r.status().as_u16() == 200 {
                    started = true;
                    break;
                }
            }
        }
        assert!(started, "waterfalls hasn't started after 5s");

        WaterfallsD {
            process,
            waterfalls_url,
        }
    }
}

impl Drop for WaterfallsD {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
