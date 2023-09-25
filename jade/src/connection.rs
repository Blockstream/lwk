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
}

impl Connection {
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Bluetooth => unimplemented!(),
            Connection::TcpStream(stream) => stream.write_all(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.write_all(buf),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Bluetooth => todo!(),
            Connection::TcpStream(stream) => stream.read(buf),

            #[cfg(feature = "serial")]
            Connection::Serial(port) => port.read(buf),
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
