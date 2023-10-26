use crate::{
    protocol::{HandshakeCompleteParams, HandshakeInitParams},
    Jade,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Jade(#[from] crate::Error),

    #[error(transparent)]
    Http(#[from] minreq::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error("Http request to {0} returned {1} instead of 200")]
    HttpStatus(String, i32),

    #[error("Jade authentication returned a response without urlA")]
    MissingUrlA,

    #[error("The handshake complete call to the pin server failed")]
    HandshakeFailed,
}

impl Jade {
    /// Unlock an already initialized Jade.
    ///
    /// The device asks for the pin,
    /// and the host performs network calls to the pin server
    /// to decrypt the secret on the device.
    pub fn unlock(&mut self) -> Result<(), Error> {
        let result = self.auth_user()?;
        let url = result.urls().get(0).ok_or(Error::MissingUrlA)?.as_str();
        let resp = minreq::post(url).send()?;
        if resp.status_code != 200 {
            return Err(Error::HttpStatus(url.to_string(), resp.status_code));
        }

        let params: HandshakeInitParams = serde_json::from_slice(resp.as_bytes())?;
        let result = self.handshake_init(params)?;
        let url = result.urls().get(0).ok_or(Error::MissingUrlA)?.as_str();
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
