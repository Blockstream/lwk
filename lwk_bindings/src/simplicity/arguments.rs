use std::collections::HashMap;
use std::sync::Arc;

use lwk_simplicity_options::simplicityhl;
use lwk_simplicity_options::utils::{
    convert_values_to_map, validate_bytes_length, SimplicityValue,
};

use crate::types::Hex;
use crate::LwkError;

macro_rules! impl_value_builder {
    ($type:ty) => {
        #[uniffi::export]
        impl $type {
            /// Create a new empty builder.
            #[uniffi::constructor]
            pub fn new() -> Arc<Self> {
                Arc::new(Self::default())
            }

            /// Add a numeric value (handles u8, u16, u32, u64).
            pub fn add_number(&self, name: String, value: u64) -> Arc<Self> {
                let mut new = self.clone();
                new.inner.insert(name, SimplicityValue::Number(value));
                Arc::new(new)
            }

            /// Add a byte array value from hex string (32 or 64 bytes).
            pub fn add_bytes(&self, name: String, value: Hex) -> Result<Arc<Self>, LwkError> {
                let bytes = value.as_ref().to_vec();
                if let Some(msg) = validate_bytes_length(bytes.len()) {
                    return Err(LwkError::Generic { msg });
                }
                let mut new = self.clone();
                new.inner.insert(name, SimplicityValue::Bytes(bytes));
                Ok(Arc::new(new))
            }
        }
    };
}

/// Builder for Simplicity program arguments.
///
/// See [`lwk_simplicity_options::simplicityhl::Arguments`] for more details.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityArguments {
    inner: HashMap<String, SimplicityValue>,
}

impl_value_builder!(SimplicityArguments);

impl SimplicityArguments {
    pub(crate) fn to_inner(&self) -> simplicityhl::Arguments {
        simplicityhl::Arguments::from(convert_values_to_map(&self.inner))
    }
}

/// Builder for Simplicity witness values.
///
/// See [`lwk_simplicity_options::simplicityhl::WitnessValues`] for more details.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityWitnessValues {
    inner: HashMap<String, SimplicityValue>,
}

impl_value_builder!(SimplicityWitnessValues);

impl SimplicityWitnessValues {
    pub(crate) fn to_inner(&self) -> simplicityhl::WitnessValues {
        simplicityhl::WitnessValues::from(convert_values_to_map(&self.inner))
    }
}
