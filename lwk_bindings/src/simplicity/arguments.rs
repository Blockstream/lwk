use std::collections::HashMap;
use std::sync::Arc;

use lwk_simplicity::simplicityhl::{
    parse::ParseFromStr, str::Identifier, Arguments, TemplateProgramWitness, Value,
    WitnessNameToValueMap, WitnessValues,
};

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
            pub fn add_value(
                &self,
                name: &str,
                value: &super::typed_value::SimplicityTypedValue,
            ) -> Arc<Self> {
                let mut new = self.clone();
                new.inner.insert(name.to_string(), value.inner().clone());
                Arc::new(new)
            }
        }
    };
}

/// Builder for Simplicity program arguments.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityArguments {
    inner: HashMap<String, Value>,
}

impl_value_builder!(SimplicityArguments);

impl SimplicityArguments {
    pub(crate) fn to_inner(&self) -> Result<Arguments, crate::LwkError> {
        Ok(Arguments::from_map(try_into_arguments_name_map(
            &self.inner,
        )?))
    }
}

/// Builder for Simplicity witness values.
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityWitnessValues {
    inner: HashMap<String, Value>,
}

impl_value_builder!(SimplicityWitnessValues);

impl SimplicityWitnessValues {
    pub(crate) fn to_inner(&self) -> Result<WitnessValues, crate::LwkError> {
        Ok(WitnessValues::from_map(try_into_witness_name_map(
            &self.inner,
        )?))
    }
}

fn try_into_witness_name_map(
    map: &HashMap<String, Value>,
) -> Result<HashMap<TemplateProgramWitness, Value>, crate::LwkError> {
    map.iter()
        .map(|(name, val)| {
            Ok((
                TemplateProgramWitness::witness_from_ident(&Identifier::parse_from_str(name)?),
                val.clone(),
            ))
        })
        .collect::<Result<_, crate::LwkError>>()
}

fn try_into_arguments_name_map(
    map: &HashMap<String, Value>,
) -> Result<HashMap<TemplateProgramWitness, Value>, crate::LwkError> {
    map.iter()
        .map(|(name, val)| {
            Ok((
                TemplateProgramWitness::parameter_from_ident(&Identifier::parse_from_str(name)?),
                val.clone(),
            ))
        })
        .collect::<Result<_, crate::LwkError>>()
}
