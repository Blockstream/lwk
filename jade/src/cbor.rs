use ciborium::Value;
use serde::{Deserialize, Deserializer, Serialize};

pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn new(vec: Vec<u8>) -> Self {
        Self(vec)
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
