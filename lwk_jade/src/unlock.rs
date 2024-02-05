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
    pub fn unlock(&mut self) -> Result<(), Error> {
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
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let resp = minreq::post(url).send()?;
                if resp.status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), resp.status_code));
                }

                let params: HandshakeInitParams = serde_json::from_slice(resp.as_bytes())?;
                let result = self.handshake_init(params)?;
                let url = result.urls().first().ok_or(Error::MissingUrlA)?.as_str();
                let data = serde_json::to_vec(result.data())?;
                let resp = minreq::post(url).with_body(data).send()?;
                if resp.status_code != 200 {
                    return Err(Error::HttpStatus(url.to_string(), resp.status_code));
                }
                let params: HandshakeCompleteParams = serde_json::from_slice(resp.as_bytes())?;

                let result = self.handshake_complete(params)?;

                if !result {
                    return Err(Error::HandshakeFailed);
                }

                Ok(())
            }
        }
    }
}
