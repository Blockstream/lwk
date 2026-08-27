//! Spawn a regtest `lightningd` (Core Lightning) process for integration tests.
//!
//! Ported from <https://github.com/RCasatta/lightningd>

use std::ffi::OsStr;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use clightningrpc::LightningRPC;
use electrsd::bitcoind::BitcoinD;
use tempfile::TempDir;

/// A running regtest `lightningd` process.
///
/// The process is killed, and the lightning-dir removed, when this is dropped.
pub struct LightningD {
    process: Child,

    /// RPC client connected to this node, over its `lightning-rpc` unix socket.
    pub client: LightningRPC,

    _lightning_dir: TempDir,
}

/// Options for [`LightningD::with_conf`].
#[non_exhaustive]
#[derive(Default)]
pub struct Conf {
    /// Extra `lightningd` command line arguments, e.g. `--bitcoin-cli=<path>`.
    ///
    /// `--network`, `--lightning-dir` and the `--bitcoin-rpc*` options are set
    /// automatically and must not be passed here.
    pub args: Vec<String>,

    /// If `true`, `lightningd`'s stdout and stderr are not suppressed.
    pub view_stdout: bool,
}

impl LightningD {
    /// Launch `lightningd` connected to the given `bitcoind`, with default options.
    pub fn new<S: AsRef<OsStr>>(exe: S, bitcoind: &BitcoinD) -> LightningD {
        LightningD::with_conf(exe, bitcoind, &Conf::default())
    }

    /// Launch `lightningd` connected to the given `bitcoind`.
    ///
    /// Waits for the node to create its RPC socket and to be synced with `bitcoind`
    /// before returning.
    pub fn with_conf<S: AsRef<OsStr>>(exe: S, bitcoind: &BitcoinD, conf: &Conf) -> LightningD {
        let lightning_dir = TempDir::new().expect("failed to create lightning-dir");

        let stdio = |view: bool| {
            if view {
                Stdio::inherit()
            } else {
                Stdio::null()
            }
        };

        let cookie = bitcoind
            .params
            .get_cookie_values()
            .expect("failed to read bitcoind cookie file")
            .expect("bitcoind cookie file must contain user:password");

        let process = Command::new(exe.as_ref())
            .arg("--network=regtest")
            .arg(format!(
                "--lightning-dir={}",
                lightning_dir.path().display()
            ))
            .arg(format!(
                "--bitcoin-rpcconnect={}",
                bitcoind.params.rpc_socket.ip()
            ))
            .arg(format!(
                "--bitcoin-rpcport={}",
                bitcoind.params.rpc_socket.port()
            ))
            .arg(format!("--bitcoin-rpcuser={}", cookie.user))
            .arg(format!("--bitcoin-rpcpassword={}", cookie.password))
            .args(&conf.args)
            .stdout(stdio(conf.view_stdout))
            .stderr(stdio(conf.view_stdout))
            .spawn()
            .expect("failed to spawn lightningd");

        let sock_path = lightning_dir.path().join("regtest").join("lightning-rpc");
        for i in 0.. {
            if sock_path.exists() {
                break;
            }
            assert!(i < 60, "lightningd hasn't created its RPC socket after 30s");
            thread::sleep(Duration::from_millis(500));
        }

        let client = LightningRPC::new(&sock_path);

        for i in 0.. {
            if let Ok(info) = client.getinfo() {
                if info.warning_bitcoind_sync.is_none() && info.warning_lightningd_sync.is_none() {
                    break;
                }
            }
            assert!(i < 60, "lightningd hasn't synced with bitcoind after 30s");
            thread::sleep(Duration::from_millis(500));
        }

        LightningD {
            process,
            client,
            _lightning_dir: lightning_dir,
        }
    }
}

impl Drop for LightningD {
    fn drop(&mut self) {
        let _ = self.client.stop();
        let _ = self.process.kill();
    }
}
