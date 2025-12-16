use std::{fmt::Display, str::FromStr};

use bip21_crate::NoExtras;
use elements::bitcoin::address::NetworkUnchecked;

#[derive(Clone, Debug)]
pub struct Bip21(String);

impl Bip21 {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parsed(&self) -> bip21_crate::Uri<'_, NetworkUnchecked, NoExtras> {
        // Safe to unwrap because we validated the string in from_str
        bip21_crate::Uri::from_str(&self.0).unwrap()
    }

    pub fn amount(&self) -> Option<u64> {
        self.parsed().amount.map(|a| a.to_sat())
    }

    pub fn label(&self) -> Option<String> {
        self.parsed().label.and_then(|l| l.try_into().ok())
    }

    pub fn message(&self) -> Option<String> {
        self.parsed().message.and_then(|m| m.try_into().ok())
    }
}

impl FromStr for Bip21 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let _bip21: bip21_crate::Uri<'_, NetworkUnchecked, NoExtras> =
            bip21_crate::Uri::from_str(s).map_err(|e| e.to_string())?;
        Ok(Self(s.to_string()))
    }
}

impl Display for Bip21 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
