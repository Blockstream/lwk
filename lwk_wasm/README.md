
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

Example is [live](https://blockstream.github.io/lwk/)

## For LWK library consumers (front-end developers)

The demo page showcasing some of the library functionalities can be run locally with:

```shell
$ cd lwk_wasm/www
$ npm install
$ npm run start
```

Open the browser at `http://localhost:8080`

Any changes in `index.html` and `index.js` are live reloaded in the browser.

Tested with:

```shell
$ node --version
v20.11.1
$ npm --version
10.2.4
```

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


Then follow the library consumer section.

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

### Publish web page

After building NPM package from previous section

```shell
$ git checkout master
$ cd lwk_wasm/www
$ npm run build
$ cd -
$ git checkout gh-pages
$ git reset --hard HEAD~1
$ git rebase master
$ cp lwk_wasm/www/dist/* docs/
$ git add docs
$ git commit -m "gh-pages: update site"
$ git push github gh-pages
```
