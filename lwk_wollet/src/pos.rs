//! POS (Point of Sale) configuration encoding/decoding utilities.
//!
//! This module provides functions to encode and decode POS configuration data
//! for URL-safe sharing, similar to the TypeScript implementation in btcpos.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::WolletDescriptor;

/// POS configuration structure for encoding/decoding.
/// This represents the configuration parameters for a Point of Sale setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct POSConfig {
    /// Descriptor (d field in JSON)
    #[serde(rename = "d")]
    pub descriptor: WolletDescriptor,
    /// Currency code (3-letter alpha3 code, c field in JSON)
    #[serde(rename = "c")]
    pub currency: String,
    /// Whether to show gear/settings button (optional, g field in JSON, defaults to false)
    #[serde(rename = "g", skip_serializing_if = "Option::is_none")]
    pub show_gear: Option<bool>,
    /// Whether to show description/note field (optional, n field in JSON, defaults to true)
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub show_description: Option<bool>,
}

impl POSConfig {
    /// Create a new POSConfig with required fields.
    /// Optional fields default to None and will be omitted from serialization.
    pub fn new(descriptor: WolletDescriptor, currency: String) -> Self {
        Self {
            descriptor,
            currency,
            show_gear: None,
            show_description: None,
        }
    }

    /// Set the show_gear field.
    pub fn with_show_gear(mut self, show_gear: bool) -> Self {
        self.show_gear = Some(show_gear);
        self
    }

    /// Set the show_description field.
    pub fn with_show_description(mut self, show_description: bool) -> Self {
        self.show_description = Some(show_description);
        self
    }
}

/// Encode POS configuration into a URL-safe base64 string.
///
/// This function serializes the configuration to JSON and encodes it using
/// URL-safe base64 encoding (replacing + with -, / with _, and removing padding).
///
/// # Arguments
/// * `descriptor` - The wallet descriptor
/// * `currency` - The 3-letter currency code (e.g., "USD")
/// * `show_gear` - Whether to show the gear/settings button
/// * `show_description` - Whether to show the description/note field
///
/// # Returns
/// A URL-safe base64 encoded string containing the configuration
pub fn encode_config(
    descriptor: &WolletDescriptor,
    currency: &str,
    show_gear: bool,
    show_description: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut config = POSConfig::new(descriptor.clone(), currency.to_string());

    // Only include optional fields if they differ from defaults
    // show_gear defaults to false, so only include if true
    if show_gear {
        config = config.with_show_gear(true);
    }
    // show_description defaults to true, so only include if false
    if !show_description {
        config = config.with_show_description(false);
    }

    let json = serde_json::to_string(&config)?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

/// Decode POS configuration from a URL-safe base64 string.
///
/// This function decodes the URL-safe base64 string, parses the JSON,
/// and validates the configuration. It applies default values for optional fields.
///
/// # Arguments
/// * `encoded` - The URL-safe base64 encoded configuration string
///
/// # Returns
/// `Some(POSConfig)` if decoding succeeds, `None` if the input is invalid
pub fn decode_config(encoded: &str) -> Option<POSConfig> {
    // Decode URL-safe base64 (replace - with +, _ with /, add padding if needed)
    let mut base64 = encoded.replace('-', "+").replace('_', "/");

    // Add padding back if needed
    while base64.len() % 4 != 0 {
        base64.push('=');
    }

    let json_bytes = URL_SAFE_NO_PAD.decode(&base64).ok()?;
    let json_str = String::from_utf8(json_bytes).ok()?;

    let mut config: POSConfig = serde_json::from_str(&json_str).ok()?;

    // Validate required fields
    if config.currency.len() != 3 {
        return None;
    }

    // Apply defaults for optional fields
    if config.show_gear.is_none() {
        config.show_gear = Some(false);
    }
    if config.show_description.is_none() {
        config.show_description = Some(true);
    }

    Some(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let descriptor_str = "ct(slip77(326412ff4dfc1123c44d3cd52f1e703e53949a5517252c607a5561e21e39d9cc),elwpkh(xpub6D1MPqdRwYThqmxXZgTa4MkuBC6i7J8h3yGvVEfrp6xCfBhkA8JbhUjoMkewAW84nu1k1kTu9SwqfhpqPuqqkG155mBx6z4tCPPLqy2vZEs/<0;1>/*))#f758dxak";
        let descriptor: WolletDescriptor = descriptor_str.parse().unwrap();
        let currency = "USD";
        let show_gear = true;
        let show_description = true;

        // Encode
        let encoded = encode_config(&descriptor, currency, show_gear, show_description).unwrap();

        // Decode
        let decoded = decode_config(&encoded).unwrap();

        // Verify roundtrip
        assert_eq!(decoded.descriptor, descriptor);
        assert_eq!(decoded.currency, currency);
        assert_eq!(decoded.show_gear, Some(true));
        assert_eq!(decoded.show_description, Some(true));
    }

    #[test]
    fn test_encode_decode_with_defaults() {
        // Use the same descriptor from the test data file that we know works
        let test_data_content = include_str!("../tests/data/pos_config.json");
        let test_data: serde_json::Value = serde_json::from_str(test_data_content).unwrap();
        let descriptor_str = test_data["d"].as_str().unwrap();
        let descriptor: WolletDescriptor = descriptor_str.parse().unwrap();
        let currency = "EUR";
        let show_gear = false; // default value
        let show_description = true; // default value

        // Encode
        let encoded = encode_config(&descriptor, currency, show_gear, show_description).unwrap();

        // Decode
        let decoded = decode_config(&encoded).unwrap();

        // Verify defaults are applied
        assert_eq!(decoded.descriptor, descriptor);
        assert_eq!(decoded.currency, currency);
        assert_eq!(decoded.show_gear, Some(false));
        assert_eq!(decoded.show_description, Some(true));
    }

    #[test]
    fn test_decode_invalid_base64() {
        assert!(decode_config("invalid_base64!@#").is_none());
    }

    #[test]
    fn test_decode_invalid_json() {
        let invalid_json = "not_json";
        let encoded = URL_SAFE_NO_PAD.encode(invalid_json);
        assert!(decode_config(&encoded).is_none());
    }

    #[test]
    fn test_decode_missing_required_fields() {
        // Missing currency
        let config = r#"{"d":"ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp"}"#;
        let encoded = URL_SAFE_NO_PAD.encode(config);
        assert!(decode_config(&encoded).is_none());

        // Invalid currency length
        let config = r#"{"d":"ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp","c":"US"}"#;
        let encoded = URL_SAFE_NO_PAD.encode(config);
        assert!(decode_config(&encoded).is_none());

        // Invalid descriptor
        let config = r#"{"d":"invalid_descriptor","c":"USD"}"#;
        let encoded = URL_SAFE_NO_PAD.encode(config);
        assert!(decode_config(&encoded).is_none());
    }

    #[test]
    fn test_pos_config_json_roundtrip() {
        // Test with the actual test data from pos_config.json (which uses abbreviated field names)
        let test_data_content = include_str!("../tests/data/pos_config.json");
        let test_data: serde_json::Value = serde_json::from_str(test_data_content).unwrap();

        // Extract values from the JSON file using abbreviated field names
        let descriptor_str = test_data["d"].as_str().unwrap();
        let descriptor: WolletDescriptor = descriptor_str.parse().unwrap();
        let currency = test_data["c"].as_str().unwrap();
        let show_gear = test_data["g"].as_bool().unwrap();
        // Note: "n" field is omitted when show_description is true (default)

        // Encode using our function
        let encoded = encode_config(&descriptor, currency, show_gear, true).unwrap();

        // Decode back
        let decoded = decode_config(&encoded).unwrap();

        // Verify the decoded config matches expected values
        assert_eq!(decoded.descriptor, descriptor);
        assert_eq!(decoded.currency, currency);
        assert_eq!(decoded.show_gear, Some(show_gear));
        assert_eq!(decoded.show_description, Some(true)); // default when "n" is omitted
    }

    #[test]
    fn test_decode_actual_encoded_string() {
        // Test decoding an actual encoded string from the JS implementation
        // This confirms that JS uses abbreviated field names: "d", "c", "g", "n"
        let encoded = "eyJkIjoiY3Qoc2xpcDc3KDMyNjQxMmZmNGRmYzExMjNjNDRkM2NkNTJmMWU3MDNlNTM5NDlhNTUxNzI1MmM2MDdhNTU2MWUyMWUzOWQ5Y2MpLGVsd3BraCh4cHViNkQxTVBxZFJ3WVRocW14WFpnVGE0TWt1QkM2aTdKOGgzeUd2VkVmcnA2eENmQmhrQThKYmhVam9Na2V3QVc4NG51MWsxa1R1OVN3cWZocHFQdXFxa0cxNTVtQng2ejR0Q1BQTHF5MnZaRXMvPDA7MT4vKikpI2Y3NThkeGFrIiwiYyI6IlVTRCIsImciOnRydWV9";

        let decoded = decode_config(encoded).unwrap();

        // Verify the decoded values match the JS implementation
        let expected_descriptor_str = "ct(slip77(326412ff4dfc1123c44d3cd52f1e703e53949a5517252c607a5561e21e39d9cc),elwpkh(xpub6D1MPqdRwYThqmxXZgTa4MkuBC6i7J8h3yGvVEfrp6xCfBhkA8JbhUjoMkewAW84nu1k1kTu9SwqfhpqPuqqkG155mBx6z4tCPPLqy2vZEs/<0;1>/*))#f758dxak";
        let expected_descriptor: WolletDescriptor = expected_descriptor_str.parse().unwrap();
        assert_eq!(decoded.descriptor, expected_descriptor);
        assert_eq!(decoded.currency, "USD");
        assert_eq!(decoded.show_gear, Some(true));
        assert_eq!(decoded.show_description, Some(true)); // default value when "n" is omitted
    }
}
