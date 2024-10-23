use crate::apdu::APDUCmdVec;
use crate::{apdu::StatusWord, client::Transport};
use ledger_transport_hid::TransportNativeHID;
use std::convert::TryFrom;
use std::error::Error;

/// Transport with the Ledger device.
pub struct TransportHID(TransportNativeHID);

impl TransportHID {
    pub fn new(t: TransportNativeHID) -> Self {
        Self(t)
    }
}

impl Transport for TransportHID {
    type Error = Box<dyn Error>;

    fn exchange(&self, cmd: &APDUCmdVec) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        self.0
            .exchange(&cmd)
            .map(|answer| {
                (
                    StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown),
                    answer.data().to_vec(),
                )
            })
            .map_err(|e| e.into())
    }
}
