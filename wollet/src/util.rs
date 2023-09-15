use elements_miniscript::elements::bitcoin::secp256k1;
use rand::thread_rng;

pub static EC: once_cell::sync::Lazy<secp256k1::Secp256k1<secp256k1::All>> =
    once_cell::sync::Lazy::new(|| {
        let mut ctx = secp256k1::Secp256k1::new();
        let mut rng = thread_rng();
        ctx.randomize(&mut rng);
        ctx
    });

pub fn ciborium_to_vec<T>(value: &T) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>>
where
    T: serde::ser::Serialize,
{
    let mut v = Vec::new();
    ciborium::ser::into_writer(value, &mut v)?;
    Ok(v)
}
