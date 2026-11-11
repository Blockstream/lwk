use crate::{DerivationPath, Error, Pset, Signer, WolletDescriptor};

use wasm_bindgen::prelude::*;

/// Context for actions interacting with Asset Management Platform version 2
#[wasm_bindgen]
pub struct Amp2 {
    inner: lwk_wollet::amp2::Amp2,
}

/// An Asset Management Platform version 2 descriptor
#[wasm_bindgen]
pub struct Amp2Descriptor {
    inner: lwk_wollet::amp2::Amp2Descriptor,
}

impl std::fmt::Display for Amp2Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Amp2Descriptor {
    /// Return the descriptor as a `WolletDescriptor`
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.descriptor().into()
    }

    /// Return the string representation of the descriptor.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Create an `Amp2Descriptor` using any `WolletDescriptor`
    ///
    /// Warning: AMP2 server only supports a limited subset of descriptors.
    /// To make sure this AMP2 descriptor can be used safely,
    /// register this with AMP2 as soon as possible.
    #[wasm_bindgen(js_name = newWithCustomDescriptor)]
    pub fn new_with_custom_descriptor(desc: &WolletDescriptor) -> Self {
        let inner = lwk_wollet::amp2::Amp2Descriptor::new_with_custom_descriptor(desc.into());
        Self { inner }
    }
}

impl Amp2Descriptor {
    pub(crate) fn inner(&self) -> lwk_wollet::amp2::Amp2Descriptor {
        self.inner.clone()
    }
}

impl From<lwk_wollet::amp2::Amp2Descriptor> for Amp2Descriptor {
    fn from(inner: lwk_wollet::amp2::Amp2Descriptor) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl Amp2 {
    /// Create a new AMP2 client
    ///
    ///  * `server_key` - The keyorigin xpub of the AMP2 server key
    ///  * `url` - The URL of the AMP2 server
    #[wasm_bindgen(js_name = new)]
    pub fn new(server_key: &str, url: &str) -> Result<Self, Error> {
        let inner = lwk_wollet::amp2::Amp2::new(server_key.into(), url.into())?;
        Ok(Self { inner })
    }

    /// Get an AMP2 wallet descriptor from the keyorigin xpub string obtained from a signer
    #[wasm_bindgen(js_name = descriptorFromStr)]
    pub fn descriptor_from_str(
        &self,
        keyorigin_xpub: &str,
        descriptor_blinding_key: &str,
    ) -> Result<Amp2Descriptor, Error> {
        Ok(self
            .inner
            .descriptor_from_str(keyorigin_xpub, descriptor_blinding_key)?
            .into())
    }

    /// Create an AMP2 descriptor ELIP153 compliant from a signer
    #[wasm_bindgen(js_name = elip153FromSigner)]
    pub fn elip153_from_signer(
        &self,
        signer: &Signer,
        account_num: u32,
    ) -> Result<Amp2Descriptor, Error> {
        Ok(self
            .inner
            .elip153_from_signer(&signer.inner, account_num)?
            .into())
    }

    /// ELIP153 `USER_PATH = m/purpose'/coin_type'/account'`
    #[wasm_bindgen(js_name = elip153UserPath)]
    pub fn elip153_user_path(&self, account_num: u32) -> Result<DerivationPath, Error> {
        Ok(self.inner.elip153_user_path(account_num)?.into())
    }

    /// ELIP153 `VIEW_PATH = m/purpose'/coin_type'/account'/server_fingerprint_masked'`
    #[wasm_bindgen(js_name = elip153ViewPath)]
    pub fn elip153_view_path(&self, account_num: u32) -> Result<DerivationPath, Error> {
        Ok(self.inner.elip153_view_path(account_num)?.into())
    }

    /// Create an AMP2 descriptor ELIP153 compliant from a signer managed externally.
    ///
    /// Caller must ensure that:
    /// * `user_keyorigin_xpub` is the keyorigin xpub derived at `Amp2::elip153_user_path()` for `account_num`
    /// * `view_keyorigin_xpub` is the keyorigin xpub derived at `Amp2::elip153_view_path()` for `account_num`
    ///
    /// **Warning**: Passing incorrect signer data can lead to creating an incorrect
    /// descriptor, which could lead to loss of funds.
    #[wasm_bindgen(js_name = elip153FromExternalSigner)]
    pub fn elip153_from_external_signer(
        &self,
        account_num: u32,
        user_keyorigin_xpub: &str,
        view_keyorigin_xpub: &str,
    ) -> Result<Amp2Descriptor, Error> {
        let (user_keysource, user_xpub) = lwk_common::keyorigin_xpub_from_str(user_keyorigin_xpub)?;
        let user_keysource = user_keysource.ok_or_else(|| {
            Error::Generic("missing keyorigin in user_keyorigin_xpub".to_string())
        })?;
        let (view_keysource, view_xpub) = lwk_common::keyorigin_xpub_from_str(view_keyorigin_xpub)?;
        let view_keysource = view_keysource.ok_or_else(|| {
            Error::Generic("missing keyorigin in view_keyorigin_xpub".to_string())
        })?;
        Ok(self
            .inner
            .elip153_from_external_signer(
                account_num,
                (user_keysource, user_xpub),
                (view_keysource, view_xpub),
            )?
            .into())
    }

    /// Register an AMP2 wallet with the AMP2 server
    pub async fn register(&self, desc: &Amp2Descriptor) -> Result<String, Error> {
        Ok(self.inner.register(desc.inner()).await?.wid)
    }

    /// Ask the AMP2 server to cosign a PSET
    pub async fn cosign(&self, pset: &Pset) -> Result<Pset, Error> {
        let pset = self.inner.cosign(&pset.clone().into()).await?.pset;
        Ok(pset.into())
    }
}
