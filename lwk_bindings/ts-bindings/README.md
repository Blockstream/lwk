# LWK TypeScript/WASM Bindings

Reference [uniffi-bindgen-react-native](https://github.com/nickkatsios/uniffi-bindgen-react-native).

Current problem: 
* The `lwk_bindings` crate has dependencies (like `rustls`) that don't compile for `wasm32-unknown-unknown`

## Usage

```bash
just ts-install

just ts-wasm

just ts-test

just ts-uniffi-check
```

## Known Issues

### 1. Library name detection bug

The `uniffi-bindgen-react-native` tool incorrectly detects the library name when the Cargo.toml has explicit `crate-type` 
(like `["staticlib", "cdylib", "rlib"]`). It looks for `liblwk_bindings.dylib` instead of `liblwk.dylib`.

**Workaround:** Create a symlink before running (on Mac):
```bash
ln -sf target/release/liblwk.dylib target/release/liblwk_bindings.dylib
```
