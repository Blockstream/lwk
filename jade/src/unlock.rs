use crate::{
    protocol::{HandshakeCompleteParams, HandshakeParams},
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
}

impl Jade {
    /// unlock an already initialized Jade, the device will ask the pin, and http calls to the
    /// pin server will be attempted to unlock the secret of the device.
    pub fn unlock(&mut self) -> Result<bool, Error> {
        let result = self.auth_user()?;
        let url = result.urls().get(0).ok_or(Error::MissingUrlA)?.as_str();
        let resp = minreq::post(url).send()?;
        if resp.status_code != 200 {
            return Err(Error::HttpStatus(url.to_string(), resp.status_code));
        }
        let params: HandshakeParams = serde_json::from_slice(resp.as_bytes())?;

        let result = self.handshake_init(params)?;
        let handshake_data = result.data();
        let data = serde_json::to_vec(&handshake_data)?;
        let url = result.urls().get(0).ok_or(Error::MissingUrlA)?.as_str();
        let resp = minreq::post(url).with_body(data).send()?;
        if resp.status_code != 200 {
            return Err(Error::HttpStatus(url.to_string(), resp.status_code));
        }
        let params: HandshakeCompleteParams = serde_json::from_slice(resp.as_bytes())?;

        let result = self.handshake_complete(params)?;

        Ok(result)
    }
}
