
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

## Test

At the moment tests are manual and not enforced in CI.

Other than rust the [`wasm-pack` tool](https://rustwasm.github.io/wasm-pack/installer/) is needed.

```shell
$ cd lwk_wasm
$ wasm-pack test --firefox # or --chrome
```

Then open the browser at http://127.0.0.1:8000, open also the dev tools to see console messages and
network requests.