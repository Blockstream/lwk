use std::{fmt, str::FromStr, sync::Arc};

use elements_miniscript::slip77::MasterBlindingKey;

use crate::{types::SecretKey, Chain, DescriptorPublicKey, LwkError, Network, Script};

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

    /// Return the [ELIP152](https://github.com/ElementsProject/ELIPs/blob/main/elip-0152.mediawiki) deterministic wallet identifier.
    pub fn dwid(&self, network: &Network) -> Result<String, LwkError> {
        Ok(self.inner.dwid(network.into())?)
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

#[uniffi::export]
impl WolletDescriptor {
    /// Same as `Signer::ss_desc` but with data obtained from a
    /// signer managed externally.
    ///
    /// Caller must ensure that:
    /// * `master_blinding_key` is derived from the signer, and wrapped in "slip77(...)", as
    ///   returned by `Signer::slip77_master_blinding_key`
    /// * `key` must be the signer keyorigin xpub, i.e. its fingerprint must be the signer
    ///    master fingerprint, and its xpub must be the one derived at path
    ///    `DerivationPath::ss_path(network, account_type, account_num)`.
    ///
    /// Passing incorrect signer data can lead to creating an incorrect
    /// descriptor, which could lead to loss of funds.
    ///
    /// **Experimental**: this API might change without notice.
    #[uniffi::constructor]
    pub fn ss_desc_from_external_signer(
        network: &Network,
        account_type: &str,
        account_num: u32,
        master_blinding_key: &str,
        key: &DescriptorPublicKey,
    ) -> Result<Arc<Self>, LwkError> {
        let account_type: lwk_common::SSAccountType = account_type
            .parse()
            .map_err(|e: &str| LwkError::Generic { msg: e.to_string() })?;
        let master_blinding_key = master_blinding_key
            .strip_prefix("slip77(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| LwkError::Generic {
                msg: "master_blinding_key must be wrapped in \"slip77(...)\"".into(),
            })?;
        let master_blinding_key = MasterBlindingKey::from_str(master_blinding_key)
            .map_err(|e| LwkError::Generic { msg: e.to_string() })?;
        let key = key.as_ref();
        let fingerprint = key.fingerprint().ok_or_else(|| LwkError::Generic {
            msg: "missing keyorigin fingerprint".into(),
        })?;
        let xpub = key.xpub().ok_or_else(|| LwkError::Generic {
            msg: "missing xpub".into(),
        })?;
        let derivation_path = key.derivation_path().ok_or_else(|| LwkError::Generic {
            msg: "missing keyorigin derivation path".into(),
        })?;
        let expected_path = lwk_common::ss_path(&network.into(), account_type, account_num)
            .map_err(|e| LwkError::Generic { msg: e.to_string() })?;
        if derivation_path != &expected_path {
            let msg = format!(
                "unexpected derivation path: expected {expected_path}, got {derivation_path}"
            );
            return Err(LwkError::Generic { msg });
        }

        let inner = lwk_wollet::WolletDescriptor::ss_desc_from_external_signer(
            &network.into(),
            account_type,
            account_num,
            master_blinding_key,
            fingerprint,
            xpub,
        )?;
        let wd = Self { inner };
        if wd.is_mainnet() != network.is_mainnet() {
            let msg = "inconsistent network and xpub".into();
            return Err(LwkError::Generic { msg });
        }
        Ok(Arc::new(wd))
    }
}

#[cfg(test)]
mod tests {
    use lwk_common::Network;

    use crate::{Chain, DerivationPath, DescriptorPublicKey, Mnemonic, Signer, WolletDescriptor};
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
        DerivationPath::ss_path(&network, "wpkh", 1).unwrap();
        let network = crate::Network::testnet();
        DerivationPath::ss_path(&network, "wpkh", 0).unwrap();
        DerivationPath::ss_path(&network, "shwpkh", 0).unwrap();
        DerivationPath::ss_path(&network, "tr", 0).unwrap();
        assert!(DerivationPath::ss_path(&network, "pkh", 0).is_err());

        let bare_mbk = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let mbk = format!("slip77({bare_mbk})");
        let fp = "73c5da0a";
        let xpub = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
        let key = DescriptorPublicKey::new(&format!("[{fp}/84'/1'/0']{xpub}")).unwrap();
        let d = WolletDescriptor::ss_desc_from_external_signer(&network, "wpkh", 0, &mbk, &key)
            .unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        assert_eq!(d.to_string(), exp);

        let ss = |n: &crate::Network,
                  type_: &str,
                  num: u32,
                  mbk: &str,
                  key: &DescriptorPublicKey|
         -> bool {
            WolletDescriptor::ss_desc_from_external_signer(n, type_, num, mbk, key).is_err()
        };
        assert!(ss(&network, "not-wpkh", 0, &mbk, &key));
        assert!(ss(&network, "wpkh", 1 << 31, &mbk, &key));
        assert!(ss(&network, "wpkh", 0, "not-mbk", &key));
        // a bare master_blinding_key, not wrapped in "slip77(...)", must be rejected
        assert!(ss(&network, "wpkh", 0, bare_mbk, &key));
        let network = crate::Network::mainnet();
        assert!(ss(&network, "wpkh", 0, &mbk, &key));

        // invalid DescriptorPublicKeys
        let network = crate::Network::testnet();
        let no_fp = DescriptorPublicKey::new(xpub).unwrap();
        assert!(ss(&network, "wpkh", 0, &mbk, &no_fp));
        let no_path = DescriptorPublicKey::new(&format!("[{fp}]{xpub}")).unwrap();
        assert!(ss(&network, "wpkh", 0, &mbk, &no_path));
        let wrong_purpose = DescriptorPublicKey::new(&format!("[{fp}/49'/1'/0']{xpub}")).unwrap();
        assert!(ss(&network, "wpkh", 0, &mbk, &wrong_purpose));
        let wrong_network = DescriptorPublicKey::new(&format!("[{fp}/84'/0'/0']{xpub}")).unwrap();
        assert!(ss(&network, "wpkh", 0, &mbk, &wrong_network));
        let wrong_account = DescriptorPublicKey::new(&format!("[{fp}/84'/1'/1']{xpub}")).unwrap();
        assert!(ss(&network, "wpkh", 0, &mbk, &wrong_account));
    }
}
