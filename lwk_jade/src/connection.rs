use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

#[derive(Debug)]
pub enum Connection {
    Bluetooth,
    TcpStream(TcpStream),

    #[cfg(feature = "serial")]
    Serial(Box<dyn serialport::SerialPort>),

    #[cfg(test)]
    PartialReadTest {
        data: Vec<u8>,
        status: usize,
    },
}

impl Connection {
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Bluetooth => unimplemented!(),
            Connection::TcpStream(stream) => stream.write_all(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.write_all(buf),

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

    use serde_cbor::Value;

    use crate::{
        protocol::{Request, Response},
        Jade,
    };

    use super::Connection;

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

        let jade = Jade::new(connection, crate::Network::LocaltestLiquid);
        let result: Value = jade.send(Request::Ping).unwrap();
        assert_eq!(result, text);
    }
}
