use core::fmt::Debug;

use crate::{apdu::StatusWord, interpreter::InterpreterError};

#[derive(thiserror::Error, Debug)]
pub enum LiquidClientError<T: Debug> {
    #[error("Client Error: {0}")]
    ClientError(String),
    #[error("Invalid PSET")]
    InvalidPsbt,
    #[error("Transport Error {0:?}")]
    Transport(T),
    #[error("Interpreter Error {0:?}")]
    Interpreter(InterpreterError),
    #[error("Device error, command {command}")]
    Device { command: u8, status: StatusWord },
    #[error("Unexpected Result, command {command}")]
    UnexpectedResult { command: u8, data: Vec<u8> },
    #[error("Invalid Response {0}")]
    InvalidResponse(String),
    #[error("Unsupported App Version")]
    UnsupportedAppVersion,
}

impl<T: Debug> From<InterpreterError> for LiquidClientError<T> {
    fn from(e: InterpreterError) -> LiquidClientError<T> {
        LiquidClientError::Interpreter(e)
    }
}
