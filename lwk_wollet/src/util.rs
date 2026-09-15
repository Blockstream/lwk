use crate::secp256k1;
use rand::thread_rng;

/// The secp256k1 context, created once and reused across the crate since creating it is expensive.
pub static EC: once_cell::sync::Lazy<secp256k1::Secp256k1<secp256k1::All>> =
    once_cell::sync::Lazy::new(|| {
        let mut ctx = secp256k1::Secp256k1::new();
        let mut rng = thread_rng();
        ctx.randomize(&mut rng);
        ctx
    });
