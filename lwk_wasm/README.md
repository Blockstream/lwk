
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

Example is [live](https://blockstream.github.io/lwk/)

## Run demo locally

```shell
$ cd lwk_wasm
$ wasm-pack build --dev --target web
$ python3 -m http.server 8080
```

## Test

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

## Build NPM Package, and example page

Requires wasm-pack, NPM and node installed

tested with:

```
$ node --version
v20.11.1
$ npm --version
10.2.4
```

Instructions:

```
cd lwk_wasm/
CARGO_PROFILE_RELEASE_OPT_LEVEL=z wasm-pack build
cd www
npm install
npm run start  # changes are live reloaded
```

## Publish web page

After building NPM package from previous section

```
$ git checkout master
$ rm -rf /tmp/docs && mkdir /tmp/docs
$ cd lwk_wasm
$ cp www/{index.html,index.js,bootstrap.js} /tmp/docs/
$ cp -r pkg /tmp/docs/
$ rm /tmp/docs/pkg/.gitignore
$ cd ..
$ git checkout gh-pages
$ git reset --hard HEAD~1
$ git rebase master
$ cp -r /tmp/docs .
$ git add docs
$ git commit -m "gh-pages: update site"
$ git push github gh-pages
```
