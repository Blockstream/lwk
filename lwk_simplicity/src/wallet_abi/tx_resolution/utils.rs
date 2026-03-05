use lwk_wollet::elements::pset::raw::ProprietaryKey;

/// Default fee rate used by `wallet-abi-0.1` runtime (sat/kvB).
pub const DEFAULT_FEE_RATE_SAT_KVB: f32 = 1000.0;

/// Maximum number of fee fixed-point iterations before failing.
pub const MAX_FEE_ITERS: usize = 8;

pub(crate) fn get_finalizer_spec_key() -> ProprietaryKey {
    ProprietaryKey::from_pset_pair(1, b"finalizer-spec".to_vec())
}

pub(crate) fn get_secrets_spec_key() -> ProprietaryKey {
    ProprietaryKey::from_pset_pair(1, b"secrets-spec".to_vec())
}
