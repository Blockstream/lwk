//! Builder types for Simplicity program arguments and witness values.

use crate::Error;

use super::typed_value::SimplicityTypedValue;

use std::collections::HashMap;

use lwk_simplicity::simplicityhl::parse::ParseFromStr;
use lwk_simplicity::simplicityhl::str::WitnessName;
use lwk_simplicity::simplicityhl::{Arguments, Value, WitnessValues};

use wasm_bindgen::prelude::*;

/// Builder for Simplicity program arguments.
///
/// See [`lwk_simplicity::simplicityhl::Arguments`] for more details.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct SimplicityArguments {
    inner: HashMap<String, Value>,
}

#[wasm_bindgen]
impl SimplicityArguments {
    /// Create a new empty arguments builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> SimplicityArguments {
        Self::default()
    }

    /// Add a typed Simplicity value. Returns the builder with the value added.
    #[wasm_bindgen(js_name = addValue)]
    pub fn add_value(mut self, name: &str, value: &SimplicityTypedValue) -> SimplicityArguments {
        self.inner.insert(name.to_string(), value.inner().clone());
        self
    }
}

impl SimplicityArguments {
    pub(crate) fn to_inner(&self) -> Result<Arguments, Error> {
        let map: HashMap<WitnessName, Value> = self
            .inner
            .iter()
            .map(|(name, val)| Ok((WitnessName::parse_from_str(name)?, val.clone())))
            .collect::<Result<_, Error>>()?;
        Ok(Arguments::from(map))
    }
}

/// Builder for Simplicity witness values.
///
/// See [`lwk_simplicity::simplicityhl::WitnessValues`] for more details.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct SimplicityWitnessValues {
    inner: HashMap<String, Value>,
}

#[wasm_bindgen]
impl SimplicityWitnessValues {
    /// Create a new empty witness values builder.
    #[wasm_bindgen(constructor)]
    pub fn new() -> SimplicityWitnessValues {
        Self::default()
    }

    /// Add a typed Simplicity value. Returns the builder with the value added.
    #[wasm_bindgen(js_name = addValue)]
    pub fn add_value(
        mut self,
        name: &str,
        value: &SimplicityTypedValue,
    ) -> SimplicityWitnessValues {
        self.inner.insert(name.to_string(), value.inner().clone());
        self
    }
}

impl SimplicityWitnessValues {
    pub(crate) fn to_inner(&self) -> Result<WitnessValues, Error> {
        let map: HashMap<WitnessName, Value> = self
            .inner
            .iter()
            .map(|(name, val)| Ok((WitnessName::parse_from_str(name)?, val.clone())))
            .collect::<Result<_, Error>>()?;
        Ok(WitnessValues::from(map))
    }
}
