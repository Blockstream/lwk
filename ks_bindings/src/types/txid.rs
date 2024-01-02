use std::str::FromStr;

use elements::hex::ToHex;

use crate::UniffiCustomTypeConverter;

#[derive(PartialEq, Eq)]
pub struct Txid {
    inner: elements::Txid,
}

impl From<elements::Txid> for Txid {
    fn from(inner: elements::Txid) -> Self {
        Txid { inner }
    }
}

uniffi::custom_type!(Txid, String);
impl UniffiCustomTypeConverter for Txid {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        let inner = elements::Txid::from_str(&val)?;
        Ok(Txid { inner })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.inner.to_hex()
    }
}
