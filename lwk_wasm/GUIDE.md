# Guidelines

Follow the [guidelines from the bindings crate](../lwk_bindings/GUIDE.md).
However for WASM/JS we need a slightly different approach.

### String

For object that have a string representation we implement `std::fmt::Display` and we expose them like that

```rust
#[wasm_bindgen(js_name = toString)]
pub fn to_string_js(&self) -> String {
    self.to_string()
}
```

### JSON

For objects that have a json representation, like the balance we provide a `toJSON()` method that must work when the caller use for example `JSON.stringify(object)`
Unfortunately `JSON.stringify` cannot serialize big integers  by default, thus we use string representation for `BigInt`.

### Entries

Since JSON doesn't support `BigInt` some object expose also the js standard `entries()` method so that the following code is possible

```js
const balance = wallet.balance();

// 1. Create a Map
const balanceMap = new Map(balance.entries());

// 2. Iterate directly in a for...of loop
for (const [currency, amount] of balance.entries()) {
  console.log(`${currency}: ${amount}`);
}

// 3. Convert to a plain object
const balanceObject = Object.fromEntries(balance.entries());
```

### Getters
Use the annotation `#[wasm_bindgen(getter = someData)]` make user code more idiomatic javascript.
That allows to expose `object.someData`.
If instead we use `#[wasm_bindgen(js_name = someData)]` we would expose `object.someData()`.

### Mutation

When a method conceptually enriches or updates a JS wrapper object, prefer mutating that object in place.

Avoid:
- accepting borrowed object and returning a new modified object. Caller may expect the parameter to be mutated.
- consuming object as parameter, this is not expected in the destination language.

This makes the API less ambiguous for JS users.

Example:

```rust
impl Pset {
    pub fn add_details(&mut self, wollet: &Wollet) -> Result<(), Error>
}
```

This should be used from JS as:

```js
pset.addDetails(wollet);
```
