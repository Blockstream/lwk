/// Adapted from
/// https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/bitcoin_client_rs/examples/ledger_hwi
use std::convert::TryFrom;
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;

use crate::apdu::APDUCmdVec;
use crate::{apdu::StatusWord, client::Transport};
use ledger_apdu::APDUAnswer;

/// Transport to communicate with the Ledger Speculos simulator.
#[derive(Debug)]
pub struct TransportTcp {
    connection: Mutex<TcpStream>,
}

impl TransportTcp {
    pub fn new(port: u16) -> Result<Self, Box<dyn Error>> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        let stream = TcpStream::connect(addr)?;
        Ok(Self {
            connection: Mutex::new(stream),
        })
    }
}

impl Transport for TransportTcp {
    type Error = Box<dyn Error>;
    fn exchange(&self, command: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        if let Ok(mut stream) = self.connection.lock() {
            let command_bytes = command.serialize();

            let mut req = vec![0u8; command_bytes.len() + 4];
            req[..4].copy_from_slice(&(command_bytes.len() as u32).to_be_bytes());
            req[4..].copy_from_slice(&command_bytes);
            stream.write_all(&req)?;

            let mut buff = [0u8; 4];
            let len = match stream.read(&mut buff)? {
                4 => u32::from_be_bytes(buff),
                _ => return Err("Invalid Length".into()),
            };

            let mut resp = vec![0u8; len as usize + 2];
            stream.read_exact(&mut resp)?;
            let answer = APDUAnswer::from_answer(resp).map_err(|_| "Invalid Answer")?;
            Ok((
                StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown),
                answer.data().to_vec(),
            ))
        } else {
            Err("unable to get lock".into())
        }
    }
}
