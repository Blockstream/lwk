use elements::hashes::{sha256t_hash_newtype, Hash, HashEngine};

sha256t_hash_newtype! {
    /// The tag of the [`InputsHash`]
    pub struct InputsTag = hash_str("BIP0352/Inputs");

    /// Commits to the transaction inputs, it makes the shared secret unique per transaction
    #[hash_newtype(forward)]
    pub struct InputsHash(_);
}

sha256t_hash_newtype! {
    /// The tag of the [`LabelHash`]
    pub struct LabelTag = hash_str("BIP0352/Label");

    /// Tweaks the spend key of an address so that a receiver can tell apart the payments
    /// made to different addresses derived from the same keys
    #[hash_newtype(forward)]
    pub struct LabelHash(_);
}

sha256t_hash_newtype! {
    /// The tag of the [`SharedSecretHash`]
    pub struct SharedSecretTag = hash_str("BIP0352/SharedSecret");

    /// Tweaks the spend key of an address, once per output paid to the same address in a
    /// given transaction
    #[hash_newtype(forward)]
    pub struct SharedSecretHash(_);
}

sha256t_hash_newtype! {
    /// The tag of the [`BlindingHash`]
    pub struct BlindingTag = hash_str("Silent-Payment-Blinding-Key/1.0");

    /// Derives the blinding key of a silent payment output, this is the Liquid specific part
    /// of the protocol: the sender blinds the output to a key that the receiver can compute
    /// back from the shared secret
    #[hash_newtype(forward)]
    pub struct BlindingHash(_);
}

impl InputsHash {
    /// `input_hash = hash_BIP0352/Inputs(outpoint_L || A)`
    pub fn compute(smallest_outpoint: &[u8; 36], sum_input_keys: &[u8; 33]) -> Self {
        let mut engine = Self::engine();
        engine.input(smallest_outpoint);
        engine.input(sum_input_keys);
        Self::from_engine(engine)
    }
}

impl LabelHash {
    /// `label_tweak = hash_BIP0352/Label(ser256(b_scan) || ser32(m))`
    pub fn compute(scan_key: &[u8; 32], m: u32) -> Self {
        let mut engine = Self::engine();
        engine.input(scan_key);
        engine.input(&m.to_be_bytes());
        Self::from_engine(engine)
    }
}

impl SharedSecretHash {
    /// `t_k = hash_BIP0352/SharedSecret(ser_P(ecdh_shared_secret) || ser32(k))`
    pub fn compute(shared_secret: &[u8; 33], k: u32) -> Self {
        let mut engine = Self::engine();
        engine.input(shared_secret);
        engine.input(&k.to_be_bytes());
        Self::from_engine(engine)
    }
}

impl BlindingHash {
    /// `blinding_key = hash_Silent-Payment-Blinding-Key/1.0(ser_P(ecdh_shared_secret) || ser32(k))`
    pub fn compute(shared_secret: &[u8; 33], k: u32) -> Self {
        let mut engine = Self::engine();
        engine.input(shared_secret);
        engine.input(&k.to_be_bytes());
        Self::from_engine(engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements::hashes::{sha256, sha256t::Tag, HashEngine};

    fn midstate(tag: &str) -> [u8; 32] {
        let tag_hash = sha256::Hash::hash(tag.as_bytes());
        let mut engine = sha256::Hash::engine();
        engine.input(&tag_hash[..]);
        engine.input(&tag_hash[..]);
        engine.midstate().to_byte_array()
    }

    #[test]
    fn tagged_hashes_midstates() {
        assert_eq!(
            InputsTag::engine().midstate().to_byte_array(),
            midstate("BIP0352/Inputs")
        );
        assert_eq!(
            LabelTag::engine().midstate().to_byte_array(),
            midstate("BIP0352/Label")
        );
        assert_eq!(
            SharedSecretTag::engine().midstate().to_byte_array(),
            midstate("BIP0352/SharedSecret")
        );
        assert_eq!(
            BlindingTag::engine().midstate().to_byte_array(),
            midstate("Silent-Payment-Blinding-Key/1.0")
        );
    }
}
