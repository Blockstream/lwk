//! Per-output silent payment key material.

use crate::secp256k1::{PublicKey, SecretKey};

/// The per-output key material for output index `k`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentPaymentOutput {
    /// `P_spend_k = B_spend + t_k·G`.
    pub spend_pubkey: PublicKey,
    /// `BK_k = bk_k·G`.
    pub blinding_pubkey: PublicKey,
    /// `bk_k`, recomputed by the receiver to unblind.
    pub blinding_seckey: SecretKey,
}

impl SilentPaymentOutput {
    /// `P_spend_k` as an x-only key.
    pub fn x_only_pubkey(&self) -> crate::elements::secp256k1_zkp::XOnlyPublicKey {
        self.spend_pubkey.x_only_public_key().0
    }

    /// The `OP_1 <x_only(P_k)>` scriptPubKey.
    pub fn script_pubkey(&self) -> crate::elements::Script {
        use crate::elements::schnorr::TweakedPublicKey;
        let tweaked = TweakedPublicKey::new(self.x_only_pubkey());
        crate::elements::Script::new_v1_p2tr_tweaked(tweaked)
    }
}
