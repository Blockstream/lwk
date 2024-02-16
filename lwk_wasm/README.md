
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

Example is [live](https://blockstream.github.io/lwk/)

## Test

At the moment tests are manual and not enforced in CI.

Other than rust the [`wasm-pack` tool](https://rustwasm.github.io/wasm-pack/installer/) is needed.

```shell
$ cd lwk_wasm
$ wasm-pack test --firefox # or --chrome
```

Then open the browser at http://127.0.0.1:8000, open also the dev tools to see console messages and
network requests.

### Headless test

To avoid requiring opening the browser the headless mode is possible.

Note the increased timeout specified via the env var, the 20s default one could be too low.

```
$ cd lwk_wasm
$ WASM_BINDGEN_TEST_TIMEOUT=60 wasm-pack test --firefox --headless
```

run specific test (note the double `--`)

```
$ wasm-pack test --firefox --headless -- -- balance_test_testnet
```

## Build & publish

```
$ git checkout master
$ cd lwk_wasm
$ CARGO_PROFILE_RELEASE_OPT_LEVEL=z wasm-pack build --target web
$ mkdir /tmp/docs
$ cp index.html /tmp/docs/
$ cp -r pkg /tmp/docs/
$ cd ..
$ git checkout gh-pages
$ git reset --hard HEAD~1
$ cp -r /tmp/docs .
$ git add docs
$ git commit -m "gh-pages: update site"
$ git push github gh-pages
```
