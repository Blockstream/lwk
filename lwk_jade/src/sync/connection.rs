use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

#[derive(Debug)]
pub enum Connection {
    #[allow(dead_code)]
    Bluetooth,

    TcpStream(TcpStream),

    #[cfg(feature = "serial")]
    Serial(Box<dyn serialport::SerialPort>),

    #[cfg(test)]
    MockJade(MockJade),
}

impl Connection {
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Bluetooth => unimplemented!(),
            Connection::TcpStream(stream) => stream.write_all(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.write_all(buf),

            #[cfg(test)]
            Connection::MockJade(mock) => mock.write_all(buf),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Bluetooth => todo!(),
            Connection::TcpStream(stream) => stream.read(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.read(buf),

            #[cfg(test)]
            Connection::MockJade(mock) => mock.read(buf),
        }
    }
}

impl From<TcpStream> for Connection {
    fn from(stream: TcpStream) -> Self {
        Connection::TcpStream(stream)
    }
}

#[cfg(feature = "serial")]
impl From<Box<dyn serialport::SerialPort>> for Connection {
    fn from(port: Box<dyn serialport::SerialPort>) -> Self {
        Connection::Serial(port)
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct MockJade {
    replies: Vec<serde_bytes::ByteBuf>,
    ids: Vec<String>,
    served: usize,
    pending: Vec<u8>,
    pub fail_next_read: bool,
    pub partial_reads: bool,
    interrupt_next_read: bool,
}

#[cfg(test)]
impl MockJade {
    pub fn new(replies: Vec<serde_bytes::ByteBuf>) -> Self {
        Self {
            replies,
            ids: vec![],
            served: 0,
            pending: vec![],
            fail_next_read: false,
            partial_reads: false,
            interrupt_next_read: false,
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let id = serde_cbor::from_slice::<crate::protocol::Response<serde_cbor::Value>>(buf)
            .unwrap()
            .id;
        self.ids.push(id);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.fail_next_read {
            self.fail_next_read = false;
            return Err(io::Error::new(io::ErrorKind::TimedOut, "read timed out"));
        }
        if self.interrupt_next_read {
            self.interrupt_next_read = false;
            return Err(io::Error::new(io::ErrorKind::Interrupted, "oh no!"));
        }

        if self.pending.is_empty() {
            let resp = crate::protocol::Response {
                id: self.ids[self.served].clone(),
                result: Some(self.replies[self.served].clone()),
                error: None,
                seqnum: None,
                seqlen: None,
            };
            self.served += 1;
            self.pending = serde_cbor::to_vec(&resp).unwrap();

            if self.partial_reads {
                buf[0] = self.pending.remove(0);
                self.interrupt_next_read = true;
                return Ok(1);
            }
        }

        let bytes = std::mem::take(&mut self.pending);
        buf[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

#[cfg(test)]
mod test {

    use lwk_common::Network;
    use serde_bytes::ByteBuf;

    use crate::{
        protocol::{GetMasterBlindingKeyParams, GetSignatureParams},
        Jade,
    };

    use super::{Connection, MockJade};

    #[test]
    fn partial_read() {
        let master_blinding_key = ByteBuf::from(vec![0; 32]);

        let mut mock = MockJade::new(vec![master_blinding_key.clone()]);
        mock.partial_reads = true;

        let jade = Jade::new(Connection::MockJade(mock), Network::default_regtest());

        let params = GetMasterBlindingKeyParams {
            only_if_silent: false,
        };
        assert_eq!(
            jade.get_master_blinding_key(params).unwrap(),
            master_blinding_key
        );
    }

    #[test]
    fn stale_reply_is_not_returned_as_a_signature() {
        let master_blinding_key = ByteBuf::from(vec![0; 32]);
        let signature = ByteBuf::from(vec![1; 64]);

        let mut mock = MockJade::new(vec![master_blinding_key, signature.clone()]);
        mock.fail_next_read = true;

        let jade = Jade::new(Connection::MockJade(mock), Network::default_regtest());

        let params = GetMasterBlindingKeyParams {
            only_if_silent: false,
        };
        assert!(jade.get_master_blinding_key(params).is_err());

        let params = GetSignatureParams {
            ae_host_entropy: vec![1u8; 32],
        };
        let signature_read = jade.get_signature_for_tx(params).unwrap();

        assert_eq!(
            signature_read, signature,
            "returned the master blinding key as a signature"
        );
    }
}
