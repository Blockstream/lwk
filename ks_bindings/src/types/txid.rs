use std::str::FromStr;

use crate::UniffiCustomTypeConverter;

#[derive(PartialEq, Eq)]
pub struct Txid {
    pub(crate) val: String,
}
impl Txid {
    pub fn txid(&self) -> elements::Txid {
        elements::Txid::from_str(&self.val).expect("enforced by invariants")
    }
}
uniffi::custom_type!(Txid, String);
impl UniffiCustomTypeConverter for Txid {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        elements::Txid::from_str(&val)?;
        Ok(Txid { val })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.val
    }
}
impl From<elements::Txid> for Txid {
    fn from(value: elements::Txid) -> Self {
        Txid {
            val: value.to_string(),
        }
    }
}
