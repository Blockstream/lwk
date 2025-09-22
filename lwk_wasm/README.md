
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

[Available](https://www.npmjs.com/package/lwk_wasm) as npm package.

For an example usage see the [Liquid Web Wallet](https://liquidwebwallet.org/) ([source](https://github.com/RCasatta/liquid-web-wallet)). Works as CT descriptor watch-only wallet or connected to a Jade.


## For LWK Library developers

To build the WASM library you need [rust](https://www.rust-lang.org/learn/get-started) and
[wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) installed

```shell
$ wasm-pack build --dev
```

To enable web-serial:

```shell
$ RUSTFLAGS="--cfg=web_sys_unstable_apis" wasm-pack build --dev --features serial
```

## For LWK library consumers (front-end developers)

Download the Liquid Web Wallet source

```shell
$ git clone https://github.com/RCasatta/liquid-web-wallet
$ npm install
$ npm run start
```

Open the browser at `http://localhost:8080`

### Test

```shell
$ cd lwk_wasm
$ wasm-pack test --firefox # or --chrome
```

Then open the browser at http://127.0.0.1:8000, open also the dev tools to see console messages and
network requests.

To avoid requiring opening the browser the headless mode is possible.

Note the increased timeout specified via the env var, the 20s default one could be too low.

```shell
$ cd lwk_wasm
$ WASM_BINDGEN_TEST_TIMEOUT=60 wasm-pack test --firefox --headless
```

run specific test (note the double `--`)

```shell
$ wasm-pack test --firefox --headless -- -- balance_test_testnet
```

### Build NPM Package for release

Build rust crates in release mode, optimizing for space.

```shell
$ cd lwk_wasm/
$ RUSTFLAGS="--cfg=web_sys_unstable_apis" CARGO_PROFILE_RELEASE_OPT_LEVEL=z wasm-pack build --features serial
```

```shell
$ cd pkg
$ npm publish
```

### Build wasm lib for profiling

To analyze the generated wasm file to optimize for size, we want to follow the same optimization
as release but we want to keep debug info to analyze the produced lib with function names.

```shell
$ cd lwk_wasm/
$ RUSTFLAGS="--cfg=web_sys_unstable_apis" CARGO_PROFILE_RELEASE_OPT_LEVEL=z CARGO_PROFILE_RELEASE_DEBUG=2 wasm-pack build --profiling --features serial
```

With [twiggy](https://github.com/rustwasm/twiggy) is then possible to analyze the library:

```shell
twiggy top -n 10 pkg/lwk_wasm_bg.wasm
```

### Build for nodejs

```shell
$ cd lwk_wasm
$ RUSTFLAGS="--cfg=web_sys_unstable_apis" CARGO_PROFILE_RELEASE_OPT_LEVEL=z wasm-pack build --target nodejs --out-dir pkg_node -- --features serial
```

Rename the package to `lwk_node` so that we can publish it to npm.

```shell
sed -i 's/"lwk_wasm"/"lwk_node"/g' pkg_node/package.json
```

### Test node js examples

Requirement:

* having built node pkg like shown in previous paragraph
* having node and npm installed

```shell
cd lwk_wasm/tests/node
npm install
node network.js
```

## Javascript code conventions

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

## Documentation

Documentation of this crate should not use link to rust types such as [`Transaction`] because they are not usable in end-user javascript packages.
Many types are wrappers of types in lwk crates, in this cases we mostly duplicate the original documentation with context adjustment. 