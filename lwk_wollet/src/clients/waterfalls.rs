use crate::{clients::History, Error};
use age::x25519::Recipient;
use base64::Engine;
use serde::Deserialize;
use std::{collections::HashMap, io::Write};

/// The result of a "waterfalls" descriptor endpoint call
#[derive(Deserialize)]
pub struct WaterfallsResult {
    pub txs_seen: HashMap<String, Vec<Vec<History>>>,
    pub page: u16,
}

/// Encrypt a plaintext using a recipient key
///
/// This can be used to encrypt a descriptor to share with a "waterfalls" server
pub fn encrypt(plaintext: &str, recipient: Recipient) -> Result<String, Error> {
    let recipients = [recipient];
    let encryptor =
        age::Encryptor::with_recipients(recipients.iter().map(|e| e as &dyn age::Recipient))
            .expect("we provided a recipient");

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|_| Error::CannotEncrypt)?;
    writer.write_all(plaintext.as_ref())?;
    writer.finish()?;
    let result = base64::prelude::BASE64_STANDARD_NO_PAD.encode(encrypted);
    Ok(result)
}
