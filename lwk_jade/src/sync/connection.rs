use std::{
    fmt,
    io::{self, Read, Write},
    net::TcpStream,
};

/// Blocking byte transport for a Jade device.
pub trait JadeTransport: Send {
    /// Write all bytes to the device or return an error.
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Read bytes from the device into the provided buffer.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

pub enum Connection {
    #[allow(dead_code)]
    Bluetooth,

    TcpStream(TcpStream),

    #[cfg(feature = "serial")]
    Serial(Box<dyn serialport::SerialPort>),

    External(Box<dyn JadeTransport>),

    #[cfg(test)]
    PartialReadTest {
        data: Vec<u8>,
        status: usize,
    },
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Connection::Bluetooth => f.write_str("Bluetooth"),
            Connection::TcpStream(stream) => f.debug_tuple("TcpStream").field(stream).finish(),

            #[cfg(feature = "serial")]
            Connection::Serial(_) => f.write_str("Serial(..)"),

            Connection::External(_) => f.write_str("External(..)"),

            #[cfg(test)]
            Connection::PartialReadTest { data, status } => f
                .debug_struct("PartialReadTest")
                .field("data", data)
                .field("status", status)
                .finish(),
        }
    }
}

impl Connection {
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Bluetooth => unimplemented!(),
            Connection::TcpStream(stream) => stream.write_all(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.write_all(buf),

            Connection::External(transport) => transport.write_all(buf),

            #[cfg(test)]
            Connection::PartialReadTest { data: _, status: _ } => Ok(()),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Bluetooth => todo!(),
            Connection::TcpStream(stream) => stream.read(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.read(buf),

            Connection::External(transport) => transport.read(buf),

            #[cfg(test)]
            Connection::PartialReadTest { data, status } => match status {
                0 => {
                    buf[0] = data[0];
                    *status = 1;
                    Ok(1)
                }
                1 => {
                    *status = 2;
                    Err(io::Error::new(io::ErrorKind::Interrupted, "oh no!"))
                }
                _ => {
                    buf[..data.len() - 1].copy_from_slice(&data[1..]);
                    Ok(data.len() - 1)
                }
            },
        }
    }
}

impl From<Box<dyn JadeTransport>> for Connection {
    fn from(transport: Box<dyn JadeTransport>) -> Self {
        Connection::External(transport)
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
mod test {
    use std::io;

    use lwk_common::Network;
    use serde_cbor::Value;

    use crate::{
        protocol::{Request, Response},
        Jade,
    };

    use super::Connection;
    use super::JadeTransport;

    #[test]
    fn partial_read() {
        let text = Value::Text("Hello".to_string());

        let resp = Response {
            id: "0".to_string(),
            result: Some(text.clone()),
            error: None,
        };
        let mut data = Vec::new();
        serde_cbor::to_writer(&mut data, &resp).unwrap();

        let connection = Connection::PartialReadTest { data, status: 0 };

        let jade = Jade::new(connection, Network::LocaltestLiquid);
        let result: Value = jade.send(Request::Ping).unwrap();
        assert_eq!(result, text);
    }

    #[test]
    fn external_transport_partial_read() {
        let text = Value::Text("Hello".to_string());

        let resp = Response {
            id: "0".to_string(),
            result: Some(text.clone()),
            error: None,
        };
        let mut data = Vec::new();
        serde_cbor::to_writer(&mut data, &resp).unwrap();

        let jade = Jade::from_transport(
            Box::new(PartialReadTransport { data, status: 0 }),
            Network::LocaltestLiquid,
        );
        let result: Value = jade.send(Request::Ping).unwrap();
        assert_eq!(result, text);
    }

    struct PartialReadTransport {
        data: Vec<u8>,
        status: usize,
    }

    impl JadeTransport for PartialReadTransport {
        fn write_all(&mut self, _bytes: &[u8]) -> io::Result<()> {
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.status {
                0 => {
                    buf[0] = self.data[0];
                    self.status = 1;
                    Ok(1)
                }
                1 => {
                    self.status = 2;
                    Err(io::Error::new(io::ErrorKind::Interrupted, "oh no!"))
                }
                _ => {
                    buf[..self.data.len() - 1].copy_from_slice(&self.data[1..]);
                    Ok(self.data.len() - 1)
                }
            }
        }
    }
}
