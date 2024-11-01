use core::fmt::Debug;

use crate::apdu::{APDUCmdVec, StatusWord};
use crate::command;
use crate::error::LiquidClientError;
use crate::interpreter::ClientCommandInterpreter;
use elements_miniscript::elements::bitcoin::consensus::encode::deserialize_partial;

pub struct LiquidClient<T: Transport> {
    transport: T,
}

impl<T: Transport> LiquidClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    async fn make_request(
        &self,
        req: &APDUCmdVec,
        interpreter: Option<&mut ClientCommandInterpreter>,
    ) -> Result<Vec<u8>, LiquidClientError<T::Error>> {
        let (mut sw, mut data) = self
            .transport
            .exchange(req)
            .await
            .map_err(LiquidClientError::Transport)?;

        if let Some(interpreter) = interpreter {
            while sw == StatusWord::InterruptedExecution {
                let response = interpreter.execute(data)?;
                let res = self
                    .transport
                    .exchange(&command::continue_interrupted(response))
                    .await
                    .map_err(LiquidClientError::Transport)?;
                sw = res.0;
                data = res.1;
            }
        }

        if sw != StatusWord::OK {
            Err(LiquidClientError::Device {
                status: sw,
                command: req.ins,
            })
        } else {
            Ok(data)
        }
    }

    /// Returns the currently running app's name, version and state flags
    pub async fn get_version(
        &self,
    ) -> Result<(String, String, Vec<u8>), LiquidClientError<T::Error>> {
        let cmd = command::get_version();
        let data = self.make_request(&cmd, None).await?;
        if data.is_empty() || data[0] != 0x01 {
            return Err(LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data,
            });
        }

        let (name, i): (String, usize) =
            deserialize_partial(&data[1..]).map_err(|_| LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data: data.clone(),
            })?;

        let (version, j): (String, usize) = deserialize_partial(&data[i + 1..]).map_err(|_| {
            LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data: data.clone(),
            }
        })?;

        let (flags, _): (Vec<u8>, usize) =
            deserialize_partial(&data[i + j + 1..]).map_err(|_| {
                LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data: data.clone(),
                }
            })?;

        Ok((name, version, flags))
    }
}

/// Asynchronous communication layer between the bitcoin client and the Ledger device.
pub trait Transport {
    type Error: Debug;
    fn exchange(
        &self,
        command: &APDUCmdVec,
    ) -> impl std::future::Future<Output = Result<(StatusWord, Vec<u8>), Self::Error>> + Send; // TODO use async in trait instead of returning Future once supported by rust
}
