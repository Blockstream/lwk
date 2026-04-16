use elements::bitcoin::bip32::{ChildNumber, DerivationPath};
use lwk_common::Signer as _;

use crate::{LwkError, Signer};

/// Wallet-owned BIP32 derivation metadata for one outpoint.
#[derive(uniffi::Record, Clone)]
pub struct WalletAbiBip32DerivationPair {
    /// Compressed ECDSA public key hex.
    pub pubkey: String,
    /// Key source string encoded as `[{fingerprint}]...`.
    pub key_source: String,
}

/// Build wallet-owned BIP32 derivation metadata from a software signer and absolute path.
#[uniffi::export]
pub fn wallet_abi_bip32_derivation_pair_from_signer(
    signer: &Signer,
    derivation_path: Vec<u32>,
) -> Result<WalletAbiBip32DerivationPair, LwkError> {
    let derivation_path = derivation_path_from_indices(derivation_path)?;
    let xpub = signer.inner.derive_xpub(&derivation_path)?;
    let fingerprint = signer.inner.fingerprint();

    Ok(WalletAbiBip32DerivationPair {
        pubkey: xpub.public_key.to_string(),
        key_source: format!("[{fingerprint}]{derivation_path}"),
    })
}

fn derivation_path_from_indices(indices: Vec<u32>) -> Result<DerivationPath, LwkError> {
    let child_numbers = indices
        .into_iter()
        .map(|index| {
            if index >= 1 << 31 {
                ChildNumber::from_hardened_idx(index - (1 << 31))
            } else {
                ChildNumber::from_normal_idx(index)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DerivationPath::from(child_numbers))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use elements::bitcoin::bip32::DerivationPath;
    use lwk_common::Signer as _;
    use lwk_wollet::ElementsNetwork;

    use super::wallet_abi_bip32_derivation_pair_from_signer;
    use crate::{Mnemonic, Network, Signer};

    #[test]
    fn wallet_abi_bip32_derivation_pair_from_signer_matches_signer_path() {
        let mnemonic = Mnemonic::new(lwk_test_util::TEST_MNEMONIC).expect("mnemonic");
        let network: Network = ElementsNetwork::default_regtest().into();
        let signer = Signer::new(&mnemonic, &network).expect("signer");
        let derivation_path = vec![84 + (1 << 31), 1 + (1 << 31), 1 << 31, 0, 5];

        let pair = wallet_abi_bip32_derivation_pair_from_signer(&signer, derivation_path)
            .expect("derive pair");

        let expected_path = DerivationPath::from_str("m/84'/1'/0'/0/5").expect("path");
        let expected_xpub = signer
            .inner
            .derive_xpub(&expected_path)
            .expect("derive xpub");

        assert_eq!(pair.pubkey, expected_xpub.public_key.to_string());
        assert_eq!(pair.key_source, "[73c5da0a]84'/1'/0'/0/5");
    }
}
