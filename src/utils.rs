use elements::bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};

/// Derive blinders as Ledger and Jade do
// TODO: add test vectors
pub fn derive_blinder(
    master_blinding_key: &elements::slip77::MasterBlindingKey,
    hash_prevouts: &elements::bitcoin::hashes::sha256d::Hash,
    vout: u32,
    is_asset_blinder: bool,
) -> Result<secp256k1_zkp::Tweak, secp256k1_zkp::Error> {
    let key: &[u8] = &master_blinding_key.0[..];
    let mut engine: HmacEngine<sha256::Hash> = HmacEngine::new(key);
    engine.input(&hash_prevouts[..]);
    let key2 = &Hmac::from_engine(engine)[..];
    let mut engine2: HmacEngine<sha256::Hash> = HmacEngine::new(key2);
    let start = if is_asset_blinder { b'A' } else { b'V' };
    let msg: [u8; 7] = [
        start,
        b'B',
        b'F',
        ((vout >> 24) & 0xff) as u8,
        ((vout >> 16) & 0xff) as u8,
        ((vout >> 8) & 0xff) as u8,
        (vout & 0xff) as u8,
    ];
    engine2.input(&msg);
    let blinder: elements::bitcoin::hashes::Hmac<sha256::Hash> = Hmac::from_engine(engine2).into();
    secp256k1_zkp::Tweak::from_slice(&blinder.into_inner())
}
