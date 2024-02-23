use crate::Error;
use lwk_wollet::hashes::hex::FromHex;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Contract {
    inner: lwk_wollet::Contract,
}

impl std::fmt::Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(&self.inner).expect("contain simple types");
        write!(f, "{}", &json)
    }
}

#[wasm_bindgen]
impl Contract {
    /// Construct a Contract object
    pub fn new(
        domain: String,
        issuer_pubkey: &str,
        name: String,
        precision: u8,
        ticker: String,
        version: u8,
    ) -> Result<Contract, Error> {
        let inner = lwk_wollet::Contract {
            entity: lwk_wollet::Entity::Domain(domain),
            issuer_pubkey: Vec::<u8>::from_hex(issuer_pubkey)?,
            name,
            precision,
            ticker,
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

#[cfg(test)]
mod tests {
    use crate::Contract;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_contract() {
        let contract = Contract::new(
            "ciao.it".to_string(),
            "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904",
            "NAME".to_string(),
            0,
            "NME".to_string(),
            0,
        )
        .unwrap();
        let expected = "{\"entity\":{\"domain\":\"ciao.it\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"NAME\",\"precision\":0,\"ticker\":\"NME\",\"version\":0}";
        assert_eq!(contract.to_string_js(), expected);
    }
}
