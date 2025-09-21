//! Liquid block header

use elements::BlockHeader as ElementsBlockHeader;

/// Wrapper over [`elements::BlockHeader`]
#[derive(uniffi::Object, Debug, Clone)]
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

#[uniffi::export]
impl BlockHeader {
    /// Get the block hash
    pub fn block_hash(&self) -> String {
        self.inner.block_hash().to_string()
    }

    /// Get the previous block hash
    pub fn prev_blockhash(&self) -> String {
        self.inner.prev_blockhash.to_string()
    }

    /// Get the merkle root
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
