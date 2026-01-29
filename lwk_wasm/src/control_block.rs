use crate::{Error, XOnlyPublicKey};

use lwk_wollet::bitcoin::taproot;

use wasm_bindgen::prelude::*;

/// A control block for Taproot script-path spending.
///
/// See [`taproot::ControlBlock`] for more details.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ControlBlock {
    inner: taproot::ControlBlock,
}

impl From<taproot::ControlBlock> for ControlBlock {
    fn from(inner: taproot::ControlBlock) -> Self {
        ControlBlock { inner }
    }
}

impl AsRef<taproot::ControlBlock> for ControlBlock {
    fn as_ref(&self) -> &taproot::ControlBlock {
        &self.inner
    }
}

#[wasm_bindgen]
impl ControlBlock {
    /// Parse a control block from serialized bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<ControlBlock, Error> {
        let inner = taproot::ControlBlock::decode(bytes)?;
        Ok(ControlBlock { inner })
    }

    /// Serialize the control block to bytes.
    pub fn serialize(&self) -> Vec<u8> {
        self.inner.serialize()
    }

    /// Get the leaf version of the control block.
    #[wasm_bindgen(js_name = leafVersion)]
    pub fn leaf_version(&self) -> u8 {
        self.inner.leaf_version.to_consensus()
    }

    /// Get the internal key of the control block.
    #[wasm_bindgen(js_name = internalKey)]
    pub fn internal_key(&self) -> XOnlyPublicKey {
        self.inner.internal_key.into()
    }

    /// Get the output key parity (0 for even, 1 for odd).
    #[wasm_bindgen(js_name = outputKeyParity)]
    pub fn output_key_parity(&self) -> u8 {
        self.inner.output_key_parity.to_u8()
    }

    /// Get the size of the control block in bytes.
    pub fn size(&self) -> u32 {
        self.inner.size() as u32
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use lwk_wollet::hashes::hex::FromHex;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_control_block_roundtrip() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::new(&bytes).unwrap();
        assert_eq!(cb.serialize(), bytes);
    }

    #[wasm_bindgen_test]
    fn test_control_block_leaf_version() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::new(&bytes).unwrap();
        assert_eq!(cb.leaf_version(), 0xc0);
    }

    #[wasm_bindgen_test]
    fn test_control_block_internal_key() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::new(&bytes).unwrap();
        let internal_key = cb.internal_key();
        assert_eq!(
            internal_key.to_hex(),
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
    }

    #[wasm_bindgen_test]
    fn test_control_block_output_key_parity() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::new(&bytes).unwrap();
        assert!(cb.output_key_parity() <= 1);
    }

    #[wasm_bindgen_test]
    fn test_control_block_size() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::new(&bytes).unwrap();
        assert_eq!(cb.size(), 33);
    }

    #[wasm_bindgen_test]
    fn test_control_block_invalid() {
        assert!(ControlBlock::new(&[]).is_err());
        assert!(ControlBlock::new(&[0; 32]).is_err());
    }
}
