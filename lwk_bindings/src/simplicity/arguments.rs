use std::collections::HashMap;
use std::sync::Arc;

use lwk_simplicity::simplicityhl;
use simplicityhl::str::WitnessName;
use simplicityhl::Value;

use super::typed_value::SimplicityTypedValue;

macro_rules! impl_value_builder {
    ($type:ty) => {
        #[uniffi::export]
        impl $type {
            /// Create a new empty builder.
            #[uniffi::constructor]
            pub fn new() -> Arc<Self> {
                Arc::new(Self::default())
            }

            /// Add a typed Simplicity value.
            pub fn add_value(&self, name: String, value: &SimplicityTypedValue) -> Arc<Self> {
                let mut new = self.clone();
                new.inner.insert(name, value.inner().clone());
                Arc::new(new)
            }
        }
    };
}

/// Builder for Simplicity program arguments.
///
/// See [`lwk_simplicity::simplicityhl::Arguments`] for more details.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityArguments {
    inner: HashMap<String, Value>,
}

impl_value_builder!(SimplicityArguments);

impl SimplicityArguments {
    pub(crate) fn to_inner(&self) -> simplicityhl::Arguments {
        let map: HashMap<WitnessName, Value> = self
            .inner
            .iter()
            .map(|(name, val)| (WitnessName::from_str_unchecked(name), val.clone()))
            .collect();
        simplicityhl::Arguments::from(map)
    }
}

/// Builder for Simplicity witness values.
///
/// See [`lwk_simplicity::simplicityhl::WitnessValues`] for more details.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityWitnessValues {
    inner: HashMap<String, Value>,
}

impl_value_builder!(SimplicityWitnessValues);

// TODO: replace `from_str_unchecked` with parse from str
impl SimplicityWitnessValues {
    pub(crate) fn to_inner(&self) -> simplicityhl::WitnessValues {
        let map: HashMap<WitnessName, Value> = self
            .inner
            .iter()
            .map(|(name, val)| (WitnessName::from_str_unchecked(name), val.clone()))
            .collect();
        simplicityhl::WitnessValues::from(map)
    }
}
