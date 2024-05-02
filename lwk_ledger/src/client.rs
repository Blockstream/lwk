use core::fmt::Debug;

use bitcoin::{bip32::Fingerprint, consensus::encode::deserialize_partial};

use crate::{
    apdu::{APDUCommand, StatusWord},
    command,
    error::LiquidClientError,
    interpreter::ClientCommandInterpreter,
};

/// LiquidClient calls and interprets commands with the Ledger Device.
pub struct LiquidClient<T: Transport> {
    transport: T,
}

impl<T: Transport> LiquidClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    fn make_request(
        &self,
        req: &APDUCommand,
        interpreter: Option<&mut ClientCommandInterpreter>,
    ) -> Result<Vec<u8>, LiquidClientError<T::Error>> {
        let (mut sw, mut data) = self
            .transport
            .exchange(req)
            .map_err(LiquidClientError::Transport)?;

        if let Some(interpreter) = interpreter {
            while sw == StatusWord::InterruptedExecution {
                let response = interpreter.execute(data)?;
                let res = self
                    .transport
                    .exchange(&command::continue_interrupted(response))
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
    pub fn get_version(&self) -> Result<(String, String, Vec<u8>), LiquidClientError<T::Error>> {
        let cmd = command::get_version();
        let data = self.make_request(&cmd, None)?;
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

    /// Retrieve the master fingerprint.
    pub fn get_master_fingerprint(&self) -> Result<Fingerprint, LiquidClientError<T::Error>> {
        let cmd = command::get_master_fingerprint();
        self.make_request(&cmd, None).and_then(|data| {
            if data.len() < 4 {
                Err(LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data,
                })
            } else {
                let mut fg = [0x00; 4];
                fg.copy_from_slice(&data[0..4]);
                Ok(Fingerprint::from(fg))
            }
        })
    }
}

/// Communication layer between the bitcoin client and the Ledger device.
pub trait Transport {
    type Error: Debug;
    fn exchange(&self, command: &APDUCommand) -> Result<(StatusWord, Vec<u8>), Self::Error>;
}
