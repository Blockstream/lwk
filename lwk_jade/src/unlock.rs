use crate::{
    protocol::{HandshakeCompleteParams, HandshakeInitParams, IsAuthResult},
    Error, Jade,
};

impl Jade {
    /// Unlock an already initialized Jade.
    ///
    /// The device asks for the pin,
    /// and the host performs network calls to the pin server
    /// to decrypt the secret on the device.
    pub async fn unlock(&mut self) -> Result<(), Error> {
        match self.auth_user()? {
            IsAuthResult::AlreadyAuth(result) => {
                if result {
                    Ok(())
                } else {
                    // Jade is not setup, and the user declined to do it on Jade screen
                    Err(Error::NotInitialized)
                }
            }
            IsAuthResult::AuthResult(result) => {
                let client = reqwest::Client::new();
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let resp = client.get(url).send().await?;
                let status_code = resp.status().as_u16();
                if status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), status_code));
                }

                let params: HandshakeInitParams =
                    serde_json::from_slice(resp.bytes().await?.as_ref())?;
                let result = self.handshake_init(params)?;
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let data = serde_json::to_vec(result.data())?;
                let resp = client.post(url).body(data).send().await?;
                let status_code = resp.status().as_u16();
                if status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), status_code));
                }
                let params: HandshakeCompleteParams =
                    serde_json::from_slice(resp.bytes().await?.as_ref())?;

                let result = self.handshake_complete(params)?;

                if !result {
                    return Err(Error::HandshakeFailed);
                }

                Ok(())
            }
        }
    }
}
