use crate::{Error, XOnlyPublicKey};

use lwk_wollet::elements::taproot;

use wasm_bindgen::prelude::*;

/// A control block for Taproot script-path spending.
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
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<ControlBlock, Error> {
        let inner = taproot::ControlBlock::from_slice(bytes)?;
        Ok(ControlBlock { inner })
    }

    /// Serialize the control block to bytes.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.serialize()
    }

    /// Get the leaf version of the control block.
    #[wasm_bindgen(getter = leafVersion)]
    pub fn leaf_version(&self) -> u8 {
        self.inner.leaf_version.as_u8()
    }

    /// Get the internal key of the control block.
    #[wasm_bindgen(getter = internalKey)]
    pub fn internal_key(&self) -> XOnlyPublicKey {
        self.inner.internal_key.into()
    }

    /// Get the output key parity (0 for even, 1 for odd).
    #[wasm_bindgen(getter = outputKeyParity)]
    pub fn output_key_parity(&self) -> u8 {
        self.inner.output_key_parity.to_u8()
    }

    /// Get the size of the control block in bytes.
    #[wasm_bindgen(getter = size)]
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
    fn test_control_block() {
        let cb_hex = "c079be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(cb_hex).unwrap();
        let cb = ControlBlock::from_bytes(&bytes).unwrap();

        assert_eq!(cb.to_bytes(), bytes);
        assert_eq!(cb.leaf_version(), 0xc0);

        let internal_key = cb.internal_key();
        assert_eq!(internal_key.to_string(), &cb_hex[2..]);

        assert!(cb.output_key_parity() <= 1);
        assert_eq!(cb.size(), 33);

        assert!(ControlBlock::from_bytes(&[]).is_err());
        assert!(ControlBlock::from_bytes(&[0; 32]).is_err());
    }
}
