use crate::Error;
use lwk_wollet::bitcoin::bip32::{ChildNumber, DerivationPath};
use std::str::FromStr;

use wasm_bindgen::prelude::*;

/// Convert a BIP32 derivation path from string to vector.
#[wasm_bindgen(js_name = derivationPathFromStr)]
pub fn derivation_path_from_str(path: &str) -> Result<Vec<u32>, Error> {
    let path = DerivationPath::from_str(path)?;
    Ok(path.into_iter().map(|c| u32::from(*c)).collect())
}

/// Convert a BIP32 derivation path from vector to string.
#[wasm_bindgen(js_name = derivationPathToStr)]
pub fn derivation_path_to_str(path: Vec<u32>) -> String {
    let path: DerivationPath = path.into_iter().map(ChildNumber::from).collect();
    path.to_string()
}
