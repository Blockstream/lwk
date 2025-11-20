use wasm_bindgen::prelude::*;

/// The bip variant for a descriptor like specified in the bips (49, 84, 87)
#[wasm_bindgen]
pub struct Bip {
    inner: lwk_common::Bip,
}

impl std::fmt::Display for Bip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<lwk_common::Bip> for Bip {
    fn from(inner: lwk_common::Bip) -> Self {
        Self { inner }
    }
}

impl From<&Bip> for lwk_common::Bip {
    fn from(value: &Bip) -> Self {
        value.inner
    }
}

impl From<Bip> for lwk_common::Bip {
    fn from(value: Bip) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl Bip {
    /// Creates a bip49 variant
    pub fn bip49() -> Bip {
        lwk_common::Bip::Bip49.into()
    }

    /// Creates a bip84 variant
    pub fn bip84() -> Bip {
        lwk_common::Bip::Bip84.into()
    }

    /// Creates a bip87 variant
    pub fn bip87() -> Bip {
        lwk_common::Bip::Bip87.into()
    }

    /// Return the string representation of the bip variant, such as "bip49", "bip84" or "bip87"
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    use crate::Bip;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_bip() {
        assert_eq!(Bip::bip49().to_string(), "bip49");
        assert_eq!(Bip::bip84().to_string(), "bip84");
        assert_eq!(Bip::bip87().to_string(), "bip87");
    }
}
