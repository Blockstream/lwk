use crate::{
    apdu::{APDUCommand, StatusWord},
    client::Transport,
};
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

    fn exchange(&self, cmd: &APDUCommand) -> Result<(StatusWord, Vec<u8>), Self::Error> {
        let apducommand = ledger_apdu::APDUCommand {
            ins: cmd.ins,
            cla: cmd.cla,
            p1: cmd.p1,
            p2: cmd.p2,
            data: cmd.data.clone(),
        };
        self.0
            .exchange(&apducommand)
            .map(|answer| {
                (
                    StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown),
                    answer.data().to_vec(),
                )
            })
            .map_err(|e| e.into())
    }
}
