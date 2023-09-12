use bip39::Mnemonic;
use elements::{
    bitcoin::{
        bip32::{self, ExtendedPrivKey, Fingerprint},
        Network,
    },
    encode::deserialize,
    pset::PartiallySignedTransaction,
    secp256k1_zkp::{All, Secp256k1},
};

#[derive(thiserror::Error, Debug)]
pub enum SignError {
    #[error(transparent)]
    Pset(#[from] elements::pset::Error),

    #[error(transparent)]
    ElementsEncode(#[from] elements::encode::Error),

    #[error(transparent)]
    Sighash(#[from] elements_miniscript::psbt::SighashError),

    #[error(transparent)]
    Base64Encode(#[from] base64::DecodeError),
}

#[derive(thiserror::Error, Debug)]
pub enum NewError {
    #[error(transparent)]
    Bip39(#[from] bip39::Error),
}

#[allow(dead_code)]
pub struct Signer<'a> {
    mnemonic: Mnemonic,
    secp: &'a Secp256k1<All>, // could be sign only, but it is hihgly likely the caller has already the All context.
}

fn fingerprint(mnemonic: &Mnemonic, secp: &Secp256k1<All>) -> Result<Fingerprint, bip32::Error> {
    let xprv = ExtendedPrivKey::new_master(Network::Bitcoin, &mnemonic.to_seed(""))?;
    let fingerprint = xprv.fingerprint(secp);
    Ok(fingerprint)
}

impl<'a> core::fmt::Debug for Signer<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signer({})",
            fingerprint(&self.mnemonic, self.secp).expect("negligible prob to panic")
        )
    }
}

impl<'a> Signer<'a> {
    pub fn new(mnemonic: &str, secp: &'a Secp256k1<All>) -> Result<Self, NewError> {
        Ok(Self {
            mnemonic: mnemonic.parse()?,
            secp,
        })
    }

    pub fn sign(&self, pset: &str) -> Result<String, SignError> {
        let _pset = psbt_from_base64(pset)?;

        todo!()
    }
}

// TODO push upstream FromStr???
fn psbt_from_base64(base64: &str) -> Result<PartiallySignedTransaction, SignError> {
    let bytes = base64::decode(base64)?;
    Ok(deserialize(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_signer() {
        let secp = Secp256k1::new();
        let signer = Signer::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", &secp).unwrap();
        assert_eq!(format!("{:?}", signer), "Signer(73c5da0a)");
        assert_eq!(
            "mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24",
            Signer::new("bad", &secp).unwrap_err().to_string()
        );
    }
}
