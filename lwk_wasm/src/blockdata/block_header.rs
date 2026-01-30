//! Liquid block header

use lwk_wollet::elements::BlockHeader as ElementsBlockHeader;

use wasm_bindgen::prelude::*;

/// A Liquid block header
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct BlockHeader {
    inner: ElementsBlockHeader,
}

impl From<ElementsBlockHeader> for BlockHeader {
    fn from(inner: ElementsBlockHeader) -> Self {
        BlockHeader { inner }
    }
}

impl From<BlockHeader> for ElementsBlockHeader {
    fn from(value: BlockHeader) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl BlockHeader {
    /// Get the block hash as a hex string
    #[wasm_bindgen(js_name = blockHash)]
    pub fn block_hash(&self) -> String {
        self.inner.block_hash().to_string()
    }

    /// Get the previous block hash as a hex string
    #[wasm_bindgen(js_name = prevBlockhash)]
    pub fn prev_blockhash(&self) -> String {
        self.inner.prev_blockhash.to_string()
    }

    /// Get the merkle root as a hex string
    #[wasm_bindgen(js_name = merkleRoot)]
    pub fn merkle_root(&self) -> String {
        self.inner.merkle_root.to_string()
    }

    /// Get the block timestamp
    pub fn time(&self) -> u32 {
        self.inner.time
    }

    /// Get the block version
    pub fn version(&self) -> u32 {
        self.inner.version
    }

    /// Get the block height
    pub fn height(&self) -> u32 {
        self.inner.height
    }
}
