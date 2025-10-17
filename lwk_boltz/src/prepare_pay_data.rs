use std::str::FromStr;

use boltz_client::boltz::CreateSubmarineResponse;
use boltz_client::Secp256k1;
use boltz_client::ToHex;
use boltz_client::{Bolt11Invoice, Keypair};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;
use crate::SwapState;
use crate::SwapType;

#[derive(Clone, Debug)]
pub struct PreparePayData {
    pub last_state: SwapState,
    pub swap_type: SwapType,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: Option<u64>,
    pub bolt11_invoice: Option<Bolt11Invoice>,
    pub create_swap_response: CreateSubmarineResponse,
    pub our_keys: Keypair,
    pub refund_address: String,
}

impl Serialize for PreparePayData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("PreparePayData", 10)?;
        state.serialize_field("last_state", &self.last_state)?;
        state.serialize_field("swap_type", &self.swap_type)?;
        state.serialize_field("fee", &self.fee)?;
        state.serialize_field(
            "bolt11_invoice",
            &self.bolt11_invoice.as_ref().map(|i| i.to_string()),
        )?;
        state.serialize_field("create_swap_response", &self.create_swap_response)?;
        // Serialize the secret key hex string for keypair recreation
        state.serialize_field("secret_key", &self.our_keys.secret_bytes().to_hex())?;
        state.serialize_field("refund_address", &self.refund_address)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PreparePayData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PreparePayDataHelper {
            last_state: SwapState,
            swap_type: SwapType,
            fee: Option<u64>,
            bolt11_invoice: Option<String>,
            create_swap_response: CreateSubmarineResponse,
            secret_key: String, // Secret key hex string
            refund_address: String,
        }

        let helper = PreparePayDataHelper::deserialize(deserializer)?;

        // Parse bolt11_invoice from string if present
        let bolt11_invoice = if let Some(invoice_str) = helper.bolt11_invoice {
            match Bolt11Invoice::from_str(&invoice_str) {
                Ok(invoice) => Some(invoice),
                Err(_) => return Err(serde::de::Error::custom("Failed to parse bolt11 invoice")),
            }
        } else {
            None
        };

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

        Ok(PreparePayData {
            swap_type: helper.swap_type,
            last_state: helper.last_state,
            fee: helper.fee,
            bolt11_invoice,
            create_swap_response: helper.create_swap_response,
            our_keys,
            refund_address: helper.refund_address,
        })
    }
}

impl PreparePayData {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_pay_data_serialization_roundtrip() {
        // Load the JSON data from the test file
        let json_data = include_str!("../tests/data/prepare_pay_response.json");

        // Deserialize the JSON into PreparePayData
        let deserialized1: PreparePayData = serde_json::from_str(json_data)
            .expect("Failed to deserialize PreparePayData from JSON");

        // Serialize it back to JSON
        let serialized = serde_json::to_string(&deserialized1)
            .expect("Failed to serialize PreparePayData to JSON");

        // Deserialize again
        let deserialized2: PreparePayData = serde_json::from_str(&serialized)
            .expect("Failed to deserialize PreparePayData from serialized JSON");

        // Compare the two deserialized versions for equality
        assert_eq!(deserialized1.last_state, deserialized2.last_state);
        assert_eq!(deserialized1.fee, deserialized2.fee);
        assert_eq!(
            deserialized1.bolt11_invoice.as_ref().map(|i| i.to_string()),
            deserialized2.bolt11_invoice.as_ref().map(|i| i.to_string())
        );
        assert_eq!(
            deserialized1.create_swap_response.id,
            deserialized2.create_swap_response.id
        );
        assert_eq!(
            deserialized1.create_swap_response.expected_amount,
            deserialized2.create_swap_response.expected_amount
        );
        assert_eq!(
            deserialized1.create_swap_response.address,
            deserialized2.create_swap_response.address
        );
        assert_eq!(
            deserialized1.our_keys.secret_bytes(),
            deserialized2.our_keys.secret_bytes()
        );
        assert_eq!(deserialized1.refund_address, deserialized2.refund_address);

        // Also test that the full structs are equal (this will test all fields)
        // Note: We can't directly compare PreparePayData due to Keypair not implementing Eq
        // But we can compare all the individual fields as above
    }
}
