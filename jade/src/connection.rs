use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use serialport::SerialPort;

#[derive(Debug)]
pub enum Connection {
    Bluetooth,
    Serial(Box<dyn SerialPort>),
    TcpStream(TcpStream),
}

impl Connection {
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Connection::Bluetooth => unimplemented!(),
            Connection::Serial(port) => port.write_all(buf),
            Connection::TcpStream(stream) => stream.write_all(buf),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Bluetooth => todo!(),
            Connection::Serial(port) => port.read(buf),
            Connection::TcpStream(stream) => stream.read(buf),
        }
    }
}

impl From<TcpStream> for Connection {
    fn from(stream: TcpStream) -> Self {
        Connection::TcpStream(stream)
    }
}

impl From<Box<dyn SerialPort>> for Connection {
    fn from(port: Box<dyn SerialPort>) -> Self {
        Connection::Serial(port)
    }
}
