use std::str::FromStr;

use boltz_client::boltz::CreateReverseResponse;
use boltz_client::util::secrets::Preimage;
use boltz_client::Keypair;
use boltz_client::Secp256k1;
use boltz_client::ToHex;
use lwk_wollet::elements;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;
use crate::SwapState;
use crate::SwapType;

#[derive(Clone, Debug)]
pub struct InvoiceData {
    pub last_state: SwapState,
    pub swap_type: SwapType,

    /// The fee of the swap provider if known
    pub fee: Option<u64>,

    pub(crate) create_reverse_response: CreateReverseResponse,

    pub(crate) our_keys: Keypair,
    pub(crate) preimage: Preimage,
    pub(crate) claim_address: elements::Address,
}

impl Serialize for InvoiceData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("InvoiceData", 8)?;
        state.serialize_field("last_state", &self.last_state)?;
        state.serialize_field("swap_type", &self.swap_type)?;
        state.serialize_field("fee", &self.fee)?;
        state.serialize_field("create_reverse_response", &self.create_reverse_response)?;
        // Serialize the secret key hex string for keypair recreation
        state.serialize_field("secret_key", &self.our_keys.secret_bytes().to_hex())?;
        // Serialize the preimage using to_string
        state.serialize_field(
            "preimage",
            &self
                .preimage
                .to_string()
                .ok_or_else(|| serde::ser::Error::custom("Preimage bytes not available"))?,
        )?;
        state.serialize_field("claim_address", &self.claim_address.to_string())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for InvoiceData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct InvoiceDataHelper {
            last_state: SwapState,
            swap_type: SwapType,
            fee: u64,
            create_reverse_response: CreateReverseResponse,
            secret_key: String, // Secret key hex string
            preimage: String,   // Preimage hex string
            claim_address: String,
        }

        let helper = InvoiceDataHelper::deserialize(deserializer)?;

        // Recreate Keypair from secret key bytes using from_seckey_slice
        let secp = Secp256k1::new();
        let our_keys = match Keypair::from_seckey_str(&secp, &helper.secret_key) {
            Ok(keypair) => keypair,
            Err(_) => {
                return Err(serde::de::Error::custom(
                    "Failed to recreate keypair from secret key",
                ))
            }
        };

        // Parse preimage from string
        let preimage = match boltz_client::util::secrets::Preimage::from_str(&helper.preimage) {
            Ok(preimage) => preimage,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse preimage")),
        };

        // Parse claim_address from string
        let claim_address = match elements::Address::from_str(&helper.claim_address) {
            Ok(address) => address,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse claim address")),
        };

        Ok(InvoiceData {
            last_state: helper.last_state,
            swap_type: helper.swap_type,
            fee: Some(helper.fee),
            create_reverse_response: helper.create_reverse_response,
            our_keys,
            preimage,
            claim_address,
        })
    }
}

impl InvoiceData {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoice_data_serialization_roundtrip() {
        // Load the JSON data from the test file
        let json_data = include_str!("../tests/data/invoice_response.json");

        // Deserialize the JSON into InvoiceData
        let deserialized1: InvoiceData =
            serde_json::from_str(json_data).expect("Failed to deserialize InvoiceData from JSON");

        // Serialize it back to JSON
        let serialized =
            serde_json::to_string(&deserialized1).expect("Failed to serialize InvoiceData to JSON");

        // Deserialize again
        let deserialized2: InvoiceData = serde_json::from_str(&serialized)
            .expect("Failed to deserialize InvoiceData from serialized JSON");

        // Compare the two deserialized versions for equality
        assert_eq!(deserialized1.last_state, deserialized2.last_state);
        assert_eq!(deserialized1.fee, deserialized2.fee);
        assert_eq!(
            deserialized1.create_reverse_response.id,
            deserialized2.create_reverse_response.id
        );
        assert_eq!(
            deserialized1.create_reverse_response.onchain_amount,
            deserialized2.create_reverse_response.onchain_amount
        );
        assert_eq!(
            deserialized1.our_keys.secret_bytes(),
            deserialized2.our_keys.secret_bytes()
        );
        assert_eq!(deserialized1.claim_address, deserialized2.claim_address);

        // Note: We can't directly compare Preimage due to its structure, but we can compare the hex strings
        assert_eq!(
            deserialized1.preimage.to_string(),
            deserialized2.preimage.to_string()
        );
    }
}
