use std::str::FromStr;

use lwk_simplicity::scripts;
use lwk_simplicity::simplicityhl;

use lwk_wollet::bitcoin::bip32::DerivationPath;
use lwk_wollet::elements;
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;

use wasm_bindgen::prelude::*;

use crate::{ControlBlock, Error, Keypair, Signer, TxOut, XOnlyPublicKey};

/// Convert bytes to hex string.
/// TODO: this is a function for convenience, it is going to be deleted after all interfaces are
/// finalized (simplicity related)
#[wasm_bindgen(js_name = bytesToHex)]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.to_hex()
}

/// Parse hex string to bytes.
pub(crate) fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, Error> {
    Ok(Vec::<u8>::from_hex(hex)?)
}

/// Parse hex string to a fixed 32-byte array.
pub(crate) fn hex_to_bytes_32(hex: &str) -> Result<[u8; 32], Error> {
    Ok(<[u8; 32]>::from_hex(hex)?)
}

/// Get the x-only public key for a given derivation path from a signer.
/// TODO: move to the Signer structure
#[wasm_bindgen(js_name = simplicityDeriveXonlyPubkey)]
pub fn simplicity_derive_xonly_pubkey(
    signer: &Signer,
    derivation_path: &str,
) -> Result<XOnlyPublicKey, Error> {
    let keypair = derive_keypair(signer, derivation_path)?;
    Ok(keypair.x_only_public_key())
}

/// Compute the Taproot control block for Simplicity script-path spending.
#[wasm_bindgen(js_name = simplicityControlBlock)]
pub fn simplicity_control_block(
    cmr_hex: &str,
    internal_key: &XOnlyPublicKey,
) -> Result<ControlBlock, Error> {
    let cmr_bytes = hex_to_bytes_32(cmr_hex)?;
    let cmr = simplicityhl::simplicity::Cmr::from_byte_array(cmr_bytes);
    let internal_key_inner = internal_key.to_simplicityhl()?;
    let control_block = scripts::control_block(cmr, internal_key_inner);
    let serialized = control_block.serialize();
    ControlBlock::new(&serialized)
}

pub(crate) fn convert_utxos(utxos: &[TxOut]) -> Vec<elements::TxOut> {
    utxos.iter().map(elements::TxOut::from).collect()
}

pub(crate) fn derive_keypair(signer: &Signer, derivation_path: &str) -> Result<Keypair, Error> {
    let path = DerivationPath::from_str(derivation_path)?;
    let derived_xprv = signer.inner.derive_xprv(&path)?;
    let keypair = elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derived_xprv.private_key,
    );
    Ok(keypair.into())
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_control_block_roundtrip() {
        let cmr_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let internal_key_hex = "0001460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let internal_key = XOnlyPublicKey::new(internal_key_hex).unwrap();

        let cb = simplicity_control_block(cmr_hex, &internal_key).unwrap();
        let serialized = cb.serialize();

        let cb_roundtrip = ControlBlock::new(&serialized).unwrap();

        assert_eq!(cb.internal_key().to_hex(), internal_key_hex);
        assert_eq!(cb_roundtrip.internal_key().to_hex(), internal_key_hex);
        assert_eq!(cb_roundtrip.serialize(), serialized);
        assert_eq!(cb.leaf_version(), cb_roundtrip.leaf_version());
        assert_eq!(cb.output_key_parity(), cb_roundtrip.output_key_parity());
    }
}
