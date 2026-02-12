# Guidelines

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
