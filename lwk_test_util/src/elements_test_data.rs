use elements_miniscript::elements::bitcoin::hashes::{hash160, Hash as _};
use elements_miniscript::elements::bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use elements_miniscript::elements::{
    OutPoint, Script, Sequence, TxIn, TxInWitness, Txid, WPubkeyHash,
};

pub struct ElementsTestData;

impl ElementsTestData {
    pub fn secret_key(byte: u8) -> SecretKey {
        SecretKey::from_slice(&[byte; 32]).unwrap()
    }

    pub fn public_key(byte: u8) -> PublicKey {
        Self::secret_key(byte).public_key(&Secp256k1::new())
    }

    pub fn txid(byte: u8) -> Txid {
        Txid::from_byte_array([byte; 32])
    }

    pub fn outpoint(txid_byte: u8, vout: u32) -> OutPoint {
        OutPoint::new(Self::txid(txid_byte), vout)
    }

    pub fn p2wpkh(secret_key: &SecretKey) -> Script {
        Self::p2wpkh_of(&secret_key.public_key(&Secp256k1::new()))
    }

    /// Builds a P2WPKH scriptPubKey for `pubkey`.
    pub fn p2wpkh_of(pubkey: &PublicKey) -> Script {
        let hash = hash160::Hash::hash(&pubkey.serialize());
        Script::new_v0_wpkh(&WPubkeyHash::from_byte_array(hash.to_byte_array()))
    }

    /// Builds a P2WPKH witness containing `pubkey`.
    pub fn p2wpkh_witness(pubkey: &PublicKey) -> Vec<Vec<u8>> {
        vec![vec![0x30; 71], pubkey.serialize().to_vec()]
    }

    /// Builds a transaction input with the supplied witness.
    pub fn input(previous_output: OutPoint, script_witness: Vec<Vec<u8>>, is_pegin: bool) -> TxIn {
        TxIn {
            previous_output,
            is_pegin,
            script_sig: Script::new(),
            sequence: Sequence::MAX,
            asset_issuance: Default::default(),
            witness: TxInWitness {
                script_witness,
                ..Default::default()
            },
        }
    }

    /// Builds a non-pegin P2WPKH input for `secret_key`.
    pub fn p2wpkh_input(previous_output: OutPoint, secret_key: &SecretKey) -> TxIn {
        let pubkey = secret_key.public_key(&Secp256k1::new());
        Self::input(previous_output, Self::p2wpkh_witness(&pubkey), false)
    }
}

#[cfg(test)]
mod tests {
    use super::ElementsTestData;

    #[test]
    fn deterministic_elements_values() {
        assert_eq!(
            ElementsTestData::outpoint(0x42, 7).txid,
            ElementsTestData::txid(0x42)
        );
        assert_eq!(
            ElementsTestData::public_key(0x21),
            ElementsTestData::secret_key(0x21)
                .public_key(&elements_miniscript::elements::bitcoin::secp256k1::Secp256k1::new())
        );
        assert!(ElementsTestData::p2wpkh(&ElementsTestData::secret_key(0x21)).is_v0_p2wpkh());

        let secret = ElementsTestData::secret_key(0x33);
        assert_eq!(
            ElementsTestData::p2wpkh(&secret),
            ElementsTestData::p2wpkh_of(&ElementsTestData::public_key(0x33))
        );

        let input = ElementsTestData::p2wpkh_input(ElementsTestData::outpoint(0x44, 0), &secret);
        assert_eq!(
            input.witness.script_witness[1],
            ElementsTestData::public_key(0x33).serialize().to_vec()
        );
        assert!(!input.is_pegin);
    }
}
