use elements::bitcoin::bip32::{DerivationPath, Xpriv};
use elements::bitcoin::NetworkKind;
use lwk_common::Network;

use crate::secp256k1::{PublicKey, SecretKey};
use crate::{Error, EC};

use super::scan::label_tweak;
use super::{SilentPaymentAddress, SilentPaymentError, SilentPaymentNetwork};

/// BIP352 purpose, as defined in BIP43
const PURPOSE: u32 = 352;

/// The keys of a silent payment wallet, as derived in BIP352.
///
/// Only the scan secret key is needed to detect payments, the spend secret key is needed to
/// spend them: a watch only wallet is expected to hold [`SilentPaymentKeys::scan_secret_key`]
/// and [`SilentPaymentKeys::spend_public_key`] only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilentPaymentKeys {
    scan: SecretKey,
    spend: SecretKey,
}

impl SilentPaymentKeys {
    /// Create from the two secret keys
    pub fn new(scan: SecretKey, spend: SecretKey) -> Self {
        Self { scan, spend }
    }

    /// Derive the keys of the given account, `m/352'/coin_type'/account'/{1,0}'/0`
    pub fn from_xprv(xprv: &Xpriv, account: u32) -> Result<Self, Error> {
        let coin_type = if xprv.network == NetworkKind::Main {
            1776
        } else {
            1
        };
        let derive = |change: u32| -> Result<SecretKey, Error> {
            let path: DerivationPath =
                format!("m/{PURPOSE}h/{coin_type}h/{account}h/{change}h/0").parse()?;
            Ok(xprv.derive_priv(&EC, &path)?.private_key)
        };
        Ok(Self {
            scan: derive(1)?,
            spend: derive(0)?,
        })
    }

    /// Derive the keys of the given account from a BIP39 mnemonic, see
    /// [`SilentPaymentKeys::from_xprv`]
    pub fn from_mnemonic(mnemonic: &str, network: Network, account: u32) -> Result<Self, Error> {
        let mnemonic: bip39::Mnemonic = mnemonic
            .parse()
            .map_err(|e: bip39::Error| Error::Generic(format!("invalid mnemonic: {e}")))?;
        let network = if network.is_mainnet() {
            NetworkKind::Main
        } else {
            NetworkKind::Test
        };
        let xprv = Xpriv::new_master(network, &mnemonic.to_seed(""))?;
        Self::from_xprv(&xprv, account)
    }

    /// The key to detect payments
    pub fn scan_secret_key(&self) -> SecretKey {
        self.scan
    }

    /// The key to spend the payments received
    pub fn spend_secret_key(&self) -> SecretKey {
        self.spend
    }

    /// The public counterpart of [`SilentPaymentKeys::scan_secret_key`]
    pub fn scan_public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&EC, &self.scan)
    }

    /// The public counterpart of [`SilentPaymentKeys::spend_secret_key`]
    pub fn spend_public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&EC, &self.spend)
    }

    /// The address to give out to receive payments
    pub fn address(&self, network: Network) -> SilentPaymentAddress {
        SilentPaymentAddress::new(
            self.scan_public_key(),
            self.spend_public_key(),
            network.into(),
        )
    }

    /// The address labelled with `m`, see [`super::SilentPaymentScanner::labelled_address`]
    pub fn labelled_address(
        &self,
        network: Network,
        m: u32,
    ) -> Result<SilentPaymentAddress, SilentPaymentError> {
        let tweak = label_tweak(&self.scan, m)?;
        let spend = self.spend_public_key().add_exp_tweak(&EC, &tweak.into())?;
        let network: SilentPaymentNetwork = network.into();
        Ok(SilentPaymentAddress::new(
            self.scan_public_key(),
            spend,
            network,
        ))
    }

    /// The secret key spending an output paid to the address labelled with `m`
    pub fn labelled_spend_secret_key(&self, m: u32) -> Result<SecretKey, SilentPaymentError> {
        let tweak = label_tweak(&self.scan, m)?;
        Ok(self.spend.add_tweak(&tweak.into())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lwk_common::Network;

    const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn derivation() {
        let keys = SilentPaymentKeys::from_mnemonic(MNEMONIC, Network::Liquid, 0).unwrap();
        let address = keys.address(Network::Liquid);
        assert_eq!(address.scan_public_key(), keys.scan_public_key());
        assert_eq!(address.spend_public_key(), keys.spend_public_key());
        assert!(address.to_string().starts_with("lqsp1q"));

        // the account and the network change the keys
        let other = SilentPaymentKeys::from_mnemonic(MNEMONIC, Network::Liquid, 1).unwrap();
        assert_ne!(keys, other);
        let testnet =
            SilentPaymentKeys::from_mnemonic(MNEMONIC, Network::TestnetLiquid, 0).unwrap();
        assert_ne!(keys, testnet);
        assert!(testnet
            .address(Network::TestnetLiquid)
            .to_string()
            .starts_with("tlqsp1q"));

        // scan and spend keys are different
        assert_ne!(keys.scan_secret_key(), keys.spend_secret_key());
    }

    #[test]
    fn labelled() {
        let keys = SilentPaymentKeys::from_mnemonic(MNEMONIC, Network::Liquid, 0).unwrap();
        let address = keys.address(Network::Liquid);
        let labelled = keys.labelled_address(Network::Liquid, 1).unwrap();
        assert_eq!(labelled.scan_public_key(), address.scan_public_key());
        assert_ne!(labelled.spend_public_key(), address.spend_public_key());

        let secret_key = keys.labelled_spend_secret_key(1).unwrap();
        assert_eq!(
            PublicKey::from_secret_key(&EC, &secret_key),
            labelled.spend_public_key()
        );
    }
}
