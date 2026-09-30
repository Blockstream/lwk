//! Reserve TCP ports to hand to daemons spawned by tests.
//!
//! Binding port 0 and dropping the listener leaves a window before the daemon binds the port, in
//! which another test can get the same port. Instead, ports come from a range below the ephemeral
//! ports, so the OS does not assign them to others in the meantime, and each port is claimed by
//! locking a file named after it in a directory shared by all test processes. The lock is held
//! until the process exits, and the OS releases it even if the process crashes.
//!
//! The reservation is the lock, not the file: files are never deleted, and a file left by an
//! exited process is unlocked, so its port can be reserved again. Deleting them would be unsafe:
//! a process could delete a file another one has just opened, and a third process creating it
//! anew would then lock a different file, with both believing they own the port.

use std::fs::File;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Mutex;

use socket2::{Domain, Socket, Type};

/// Below the ephemeral range on Linux (32768), macOS and Windows (49152).
const PORT_FIRST: u16 = 20_000;
const PORT_LAST: u16 = 29_999;

/// Lock files of the ports reserved by this process. Statics are never dropped, so the files stay
/// open, and locked, until the process exits and the OS closes them.
static HELD: Mutex<Vec<File>> = Mutex::new(Vec::new());

/// Returns a port that no other test process has reserved and that is currently bindable.
///
/// The port stays reserved until the process exits. Reservations are coordinated through lock
/// files in `std::env::temp_dir()`, so processes with a different temporary directory do not see
/// each other's reservations.
pub fn reserve_port() -> u16 {
    let dir = std::env::temp_dir().join("lwk-test-ports");
    std::fs::create_dir_all(&dir).unwrap();

    // Start at a random offset, so concurrent processes don't contend for the same locks.
    let span = PORT_LAST - PORT_FIRST + 1;
    let start = rand::random::<u16>() % span;
    for i in 0..span {
        let port = PORT_FIRST + (start + i) % span;
        if let Some(file) = try_reserve(&dir, port) {
            HELD.lock().unwrap().push(file);
            return port;
        }
    }
    panic!("test port range exhausted");
}

/// Locks the file for `port` in `dir`, returning it if the lock is taken and the port is bindable.
fn try_reserve(dir: &Path, port: u16) -> Option<File> {
    // Opening fails if the file belongs to another user, whose tests may be using the port.
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(port.to_string()))
        .ok()?;
    file.try_lock().ok()?;
    bindable(port).then_some(file)
}

/// Whether a daemon could bind `port`, either on loopback or on all interfaces.
///
/// This catches ports used by programs not taking part in the reservation. The socket doesn't set
/// `SO_REUSEADDR` (std's `TcpListener` sets it on Unix), so a port still in TIME_WAIT reads as
/// taken, which is what a daemon not using address reuse will find too.
fn bindable(port: u16) -> bool {
    [Ipv4Addr::LOCALHOST, Ipv4Addr::UNSPECIFIED]
        .into_iter()
        .all(|ip| {
            let address = SocketAddr::from((ip, port));
            Socket::new(Domain::IPV4, Type::STREAM, None)
                .and_then(|socket| socket.bind(&address.into()))
                .is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::net::TcpListener;

    #[test]
    fn test_reserve_port_distinct() {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..5).map(|_| reserve_port()).collect::<Vec<_>>()))
            .collect();
        let ports: Vec<u16> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        let unique: HashSet<u16> = ports.iter().copied().collect();
        assert_eq!(unique.len(), ports.len());
        assert!(ports.iter().all(|p| (PORT_FIRST..=PORT_LAST).contains(p)));
    }

    #[test]
    fn test_try_reserve_skips_locked() {
        let dir = tempfile::tempdir().unwrap();
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let held = try_reserve(dir.path(), port).unwrap();
        assert!(try_reserve(dir.path(), port).is_none());
        drop(held);
        assert!(try_reserve(dir.path(), port).is_some());
    }

    #[test]
    fn test_try_reserve_skips_bound() {
        let dir = tempfile::tempdir().unwrap();
        for ip in ["127.0.0.1:0", "0.0.0.0:0"] {
            let listener = TcpListener::bind(ip).unwrap();
            let port = listener.local_addr().unwrap().port();
            assert!(try_reserve(dir.path(), port).is_none());
        }
    }
}
