# Guidelines

## Rust interface
This crate has a Rust interface, however that's not the focus.
What we care about are the interfaces in the destination languages.
For this reason we don't necessarily follow Rust guidelines.

## Docs
Documentation of this crate should not use link to rust types such as [`elements::Transaction`] because they are not usable in end-user languages.
Many types are wrappers of types in LWK crates, in this cases we mostly duplicate the original documentation with context adjustment.

## Tests
Rust unit tests are welcome, however testing the Rust intermediate interface is not enough.
We must have coverage also from a destination language.
Python is a common choice for tests due to its simplicity and popularity.
Tests in destination languages also serve as examples, try to make them useful for devs using that language.
When a function is not self-explanatory or a flow is complex, add comments; name variables to document function parameters.

## Constructors
Do not use the default constructor `new()` if there are multiple ways in which an object can be created.
This avoids ambiguity. Use constructors names that explicitly mention the format of what should be passed in.

For instance, builders can use `new()` to initialize the builder as empty/default.
However objects that can appear both in bytes and string, must not use `new()`.

## (De)serialization
When using `#[derive(uniffi::Object)]` on a rust struct follow these conventions:

### Bytes
If the object has a bytes representation:
```
impl MyType {
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> { }

    fn to_bytes(&self) -> Vec<u8> {}
}
```

Add a _python_ test to check that they roundtrip.

### String
If the object has a natural string representation, implement `Display` and add `#[uniffi::export(Display)]`

For constructors, be explicit about the string format necessary (implement a constructor from string):
```
impl MyType {
    // explicitly comment if hex-reversed, eg
    /// Note: hex representation is byte-reversed
    #[uniffi::constructor]
    pub fn from_hex(s: &str) -> Result<Arc<Self>, LwkError> { }

    #[uniffi::constructor]
    pub fn from_b64(s: &str) -> Result<Arc<Self>, LwkError> { }
}
```

Add a _python_ test to check that serialization roundtrip
