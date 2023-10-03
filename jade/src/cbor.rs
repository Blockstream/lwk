use ciborium::Value;
use serde::{Deserialize, Deserializer, Serialize};

pub struct Bytes(Vec<u8>);

// TODO impl also fixed size Bytes with const generic

impl Bytes {
    pub fn new(vec: Vec<u8>) -> Self {
        Self(vec)
    }
}

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex = hex::encode(&self.0); // TODO avoid alloc
        write!(f, "Bytes({hex})")
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(value: Bytes) -> Self {
        value.0
    }
}

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = Value::Bytes(self.0.clone()); // TODO serialize without clone
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(d)?;
        match value {
            Value::Bytes(bytes) => Ok(Bytes(bytes)),
            _ => Err(serde::de::Error::custom(
                "expecting bytes, found something else",
            )),
        }
    }
}
