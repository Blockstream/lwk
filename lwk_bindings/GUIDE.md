# Guidelines

## Rust interface
This crate has a Rust interface, however that's not the focus.
What we care about are the interfaces in the destination languages.
For this reason we don't necessarily follow Rust guidelines.

## Docs
Documentation of this crate should not use link to rust types such as [`elements::Transaction`] because they are not usable in end-user languages.
Many types are wrappers of types in LWK crates, in this cases we mostly duplicate the original documentation with context adjustment.

If a function is complex or has non-obvious behavior, add extra caller-facing context (for example by copying/adapting the relevant explanation from upstream docs).

## Tests
Rust unit tests are welcome, however testing the Rust intermediate interface is not enough.
We must have coverage also from a destination language, and we should treat that coverage as required for interface changes.

Python is a common choice for tests due to its simplicity and popularity.

Tests in destination languages also serve as examples, try to make them useful for devs using that language.

When adding/changing API surface, include destination-language checks for expected behavior and roundtrip consistency when serialization is involved.

## Function/method arguments
Always accept a function argument by an immutable reference.

This is an example of how NOT to do it: `fn some_func(arr: Vec<Arc<TxOut>>)`; instead, do: `fn some_func(arr: &[Arc<TxOut>])`.

This is because, in the targeted language, we won't have any borrowing checks, but in Rust, when the Rust compiler sees that the argument is now owned by the function, it could modify or destroy it. Therefore, if you use it again in the target language, you would have a headache debugging why the value is now broken.

Note that this sometimes implies a performance penalty, with some extra clones.
This is a deliberate choice to avoid the situation we described above.
If performance really matters, consider using the Rust crates directly.

### Exceptions
There are situations where it’s acceptable to bypass this rule.
Below is a non-exhaustive list of such cases:
- When the type implements the `Copy` trait, there is no need to add a reference. 
  - This applies to all types like `u32`, `u64`, etc.
  - This is ok: `pub fn from_height(height: u32)`
- When the builder type is consumed
  - This is ok: `SomeBuilder::add_data(self) -> SomeBuilder`
  - This is WRONG: `SomeBuilder::add_data(self, Vec<Arc<TxOut>>)`. The second argument should be `&[Arc<TxOut>]`.
- When the argument is a shared-ownership smart pointer (e.g. `Arc<T>`) and the callee needs to retain it (store it / keep a handle).
  - In this case, taking it by value is acceptable since cloning is cheap and the intent is to keep a shared owner.
  - This is ok: `LoggingLink::new(logging: Arc<dyn Logging>)`
- Or you have another **strong** reason for it
  - Make sure to explicitly state it in the end-user-facing documentation to prevent misuse.

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

Use a single canonical string interface for parsing/serialization:
```
impl MyType {
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> { }
}
```

`to_string()` (via `Display`) must produce the format accepted by `from_string`.

Document any non-obvious detail of that format in the constructor doc comment (for example if `to_string()` returns bytes in a reverse order).

Add a _python_ test to check serialization roundtrip.

## Deprecating functions
If there are functions that contradict the guidelines above and should be marked as deprecated, add the following comment:
```
Deprecated: use `function_name()` instead.
```
We do not use deprecation macros because they are ignored in the targeted bindings:

Make sure that deprecated functions are not referenced in examples/tests by running CI or local tests without them.