use crate::Error;
use lwk_wollet::hashes::hex::FromHex;
use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::Contract`]
#[wasm_bindgen]
pub struct Contract {
    inner: lwk_wollet::Contract,
}

impl From<Contract> for lwk_wollet::Contract {
    fn from(value: Contract) -> Self {
        value.inner
    }
}

impl From<lwk_wollet::Contract> for Contract {
    fn from(inner: lwk_wollet::Contract) -> Self {
        Self { inner }
    }
}

impl std::fmt::Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.inner).expect("contain simple types");
        write!(f, "{}", &json)
    }
}

#[wasm_bindgen]
impl Contract {
    /// Creates a `Contract`
    #[wasm_bindgen(constructor)]
    pub fn new(
        domain: &str,
        issuer_pubkey: &str,
        name: &str,
        precision: u8,
        ticker: &str,
        version: u8,
    ) -> Result<Contract, Error> {
        let inner = lwk_wollet::Contract {
            entity: lwk_wollet::Entity::Domain(domain.to_string()),
            issuer_pubkey: Vec::<u8>::from_hex(issuer_pubkey)?,
            name: name.to_string(),
            precision,
            ticker: ticker.to_string(),
            version,
        };
        inner.validate()?; // TODO validate should be the constructor
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::Contract;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_contract() {
        let contract = Contract::new(
            "ciao.it",
            "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904",
            "NAME",
            0,
            "NME",
            0,
        )
        .unwrap();
        let expected = "{\"entity\":{\"domain\":\"ciao.it\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"NAME\",\"precision\":0,\"ticker\":\"NME\",\"version\":0}";
        assert_eq!(contract.to_string_js(), expected);
    }
}
