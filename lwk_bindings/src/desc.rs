use elements::bitcoin::bip32::{ChildNumber, DerivationPath};
use std::{fmt, str::FromStr, sync::Arc};

use crate::{types::SecretKey, Chain, LwkError, Network, Script};

/// The output descriptors, wrapper over [`lwk_wollet::WolletDescriptor`]
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WolletDescriptor {
    inner: lwk_wollet::WolletDescriptor,
}

impl AsRef<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn as_ref(&self) -> &lwk_wollet::WolletDescriptor {
        &self.inner
    }
}

impl From<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn from(inner: lwk_wollet::WolletDescriptor) -> Self {
        Self { inner }
    }
}

impl From<&WolletDescriptor> for lwk_wollet::WolletDescriptor {
    fn from(desc: &WolletDescriptor) -> Self {
        desc.inner.clone()
    }
}

#[uniffi::export]
impl WolletDescriptor {
    /// Create a new descriptor from its string representation.
    #[uniffi::constructor]
    pub fn new(descriptor: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::WolletDescriptor::from_str(descriptor)?;
        Ok(Arc::new(WolletDescriptor { inner }))
    }

    /// Whether the descriptor is on the mainnet
    pub fn is_mainnet(&self) -> bool {
        self.inner.is_mainnet()
    }

    /// Derive the private blinding key
    pub fn derive_blinding_key(&self, script_pubkey: &Script) -> Option<Arc<SecretKey>> {
        self.inner
            .ct_descriptor()
            .map(|d| lwk_common::derive_blinding_key(d, &script_pubkey.into()))
            .ok()
            .flatten()
            .map(Into::into)
            .map(Arc::new)
    }

    /// Derive a scriptpubkey
    pub fn script_pubkey(&self, ext_int: Chain, index: u32) -> Result<Arc<Script>, LwkError> {
        self.inner
            .script_pubkey(ext_int.into(), index)
            .map_err(Into::into)
            .map(Into::into)
            .map(Arc::new)
    }

    /// Whether the descriptor is AMP0
    pub fn is_amp0(&self) -> bool {
        self.inner.is_amp0()
    }

    /// Return the descriptor encoded so that can be part of an URL
    pub fn url_encoded_descriptor(&self) -> Result<String, LwkError> {
        Ok(self.inner.url_encoded_descriptor()?)
    }
}

impl fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

fn get_path_inner(
    network: &Network,
    account_type: &str,
    account_num: u32,
) -> Result<(Vec<u32>, String, String), LwkError> {
    let coin_type = if network.is_mainnet() { 1776 } else { 1 };
    let (purpose, prefix, suffix) = match account_type {
        "wpkh" => (84, "elwpkh", ""),
        "shwpkh" => (49, "elsh(wpkh", ")"),
        "pkh" => (44, "elpkh", ""),
        "tr" => (86, "eltr", ""),
        _ => {
            return Err(LwkError::Generic {
                msg: "invalid account type, must be 'wpkh', 'shwpkh', 'pkh' or 'tr'".into(),
            })
        }
    };
    let h = 1 << 31;
    if account_num >= h {
        return Err(LwkError::Generic {
            msg: "invalid account number".into(),
        });
    }

    let path = vec![purpose + h, coin_type + h, account_num + h];
    Ok((path, prefix.into(), suffix.into()))
}

/// Get the derivation path for an account
#[uniffi::export]
pub fn get_path(
    network: &Network,
    account_type: &str,
    account_num: u32,
) -> Result<Vec<u32>, LwkError> {
    let (path, _, _) = get_path_inner(network, account_type, account_num)?;
    Ok(path)
}

#[uniffi::export]
impl WolletDescriptor {
    /// Descriptor from xpub
    ///
    /// This should be used when the xpub is obtained from a signer
    /// (e.g. Jade) managed outside LWK.
    ///
    /// If master blinding key is SLIP77, it must be wrapped in "slip77(...)"
    #[uniffi::constructor]
    pub fn from_xpub(
        network: &Network,
        account_type: &str,
        account_num: u32,
        master_blinding_key: &str,
        fingerprint: &str,
        xpub: &str,
    ) -> Result<Arc<Self>, LwkError> {
        let (path, prefix, suffix) = get_path_inner(network, account_type, account_num)?;
        let path: DerivationPath = path.into_iter().map(ChildNumber::from).collect();
        let desc = format!(
            "ct({master_blinding_key},{prefix}([{fingerprint}/{path}]{xpub}/<0;1>/*){suffix})"
        );
        let wd = Self::new(&desc)?;
        if wd.is_mainnet() != network.is_mainnet() {
            let msg = "inconsistent network and xpub".into();
            return Err(LwkError::Generic { msg });
        }
        Ok(wd)
    }
}

#[cfg(test)]
mod tests {
    use lwk_common::Network;

    use super::*;
    use crate::{Chain, Mnemonic, Signer, WolletDescriptor};
    use std::str::FromStr;

    #[test]
    fn wpkh_slip77_descriptor() {
        let mnemonic_str = lwk_test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = Network::default_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        assert_eq!(signer.wpkh_slip77_descriptor().unwrap().to_string(), exp);

        let wollet_desc = lwk_wollet::WolletDescriptor::from_str(exp).unwrap();
        let desc: WolletDescriptor = wollet_desc.into();
        assert_eq!(desc.to_string(), exp);

        assert!(!desc.is_mainnet());

        assert_eq!(
            desc.script_pubkey(Chain::External, 0).unwrap().to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );

        assert_eq!(
            desc.script_pubkey(Chain::Internal, 0).unwrap().to_string(),
            "00142f34aa1cf00a53b055a291a03a7d45f0a6988b52"
        );
    }

    #[test]
    fn separate_signer_flow() {
        let network = crate::Network::mainnet();
        get_path(&network, "wpkh", 1).unwrap();
        let network = crate::Network::testnet();
        get_path(&network, "wpkh", 0).unwrap();
        get_path(&network, "shwpkh", 0).unwrap();
        get_path(&network, "pkh", 0).unwrap();
        get_path(&network, "tr", 0).unwrap();

        let mbk = "slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023)";
        let fp = "73c5da0a";
        let xpub = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
        let d = WolletDescriptor::from_xpub(&network, "wpkh", 0, mbk, fp, xpub).unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        assert_eq!(d.to_string(), exp);

        assert!(WolletDescriptor::from_xpub(&network, "not-wpkh", 0, mbk, fp, xpub).is_err());
        assert!(WolletDescriptor::from_xpub(&network, "wpkh", 1 << 31, mbk, fp, xpub).is_err());
        assert!(WolletDescriptor::from_xpub(&network, "wpkh", 0, "not-mbk", fp, xpub).is_err());
        assert!(WolletDescriptor::from_xpub(&network, "wpkh", 0, mbk, "not-fp", xpub).is_err());
        assert!(WolletDescriptor::from_xpub(&network, "wpkh", 0, mbk, fp, "not-xpub").is_err());
        let network = crate::Network::mainnet();
        assert!(WolletDescriptor::from_xpub(&network, "wpkh", 0, mbk, fp, xpub).is_err());
    }
}
