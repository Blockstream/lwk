use anyhow::Context;
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

    pub fn new<S: AsRef<OsStr>>(exe: S, esplora_url: &str) -> anyhow::Result<WaterfallsD> {
        let addr = TcpListener::bind(("0.0.0.0", 0))?; // 0 means the OS choose a free port
        let addr = format!("{}", addr.local_addr().unwrap());

        let mut args = vec![];

        args.push("--network");
        args.push("--elements-regtest");
        args.push("--use-esplora");
        args.push("--esplora-url");
        args.push(esplora_url);
        args.push("--listen");
        args.push(&addr);
        let waterfalls_url = format!("http://{addr}");
        println!("{:?}", args);
        assert!(false);

        let mut process = Command::new(&exe)
            .args(args)
            .spawn()
            .with_context(|| format!("Error while executing {:?}", exe.as_ref()))?;

        match process.try_wait() {
            Ok(Some(_)) | Err(_) => {
                // Process has exited or an error occurred, kill and retry
                let _ = process.kill();
                panic!("failed to start waterfalls");
            }
            Ok(None) => {} // Process is still running, proceed
        }

        std::thread::sleep(Duration::from_millis(1000));

        Ok(WaterfallsD {
            process,
            waterfalls_url,
        })
    }
}

impl Drop for WaterfallsD {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
