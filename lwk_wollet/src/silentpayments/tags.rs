use crate::hashes::sha256t_hash_newtype;

sha256t_hash_newtype! {
    pub(crate) struct InputsTag = hash_str("BIP0352/Inputs");
    /// `input_hash = H_BIP0352/Inputs(outpoint_L || A)`.
    #[hash_newtype(forward)]
    pub(crate) struct InputsHash(_);

    pub(crate) struct SharedSecretTag = hash_str("BIP0352/SharedSecret");
    /// `t_k = H_BIP0352/SharedSecret(serP(S) || ser32(k))`.
    #[hash_newtype(forward)]
    pub(crate) struct SharedSecretHash(_);

    pub(crate) struct BlindTag = hash_str("LiquidSilentPayments/Blind");
    /// `bk_k = H_LiquidSilentPayments/Blind(serP(S) || ser32(k))`.
    ///
    /// Shares its preimage with [`SharedSecretHash`]; only the tag makes `bk_k` and
    /// `t_k` independent.
    #[hash_newtype(forward)]
    pub(crate) struct BlindHash(_);

    pub(crate) struct LabelTag = hash_str("BIP0352/Label");
    /// `label_tweak_m = H_BIP0352/Label(ser256(b_scan) || ser32(m))`.
    #[hash_newtype(forward)]
    pub(crate) struct LabelHash(_);
}
