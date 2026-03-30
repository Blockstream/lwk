
# Liquid Wallet Kit for WASM

This is only a proof of concept at the moment but we want to show our commitment to have the 
Liquid Wallet Kit working in the WASM environment.

[Available](https://www.npmjs.com/search?q=%40blockstream%2Flwk) as two npm packages:

- `@blockstream/lwk-web` for browser and bundler builds
- `@blockstream/lwk-node` for Node.js

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

Install the package you need.

Browser / bundler entrypoint:

```shell
$ npm install @blockstream/lwk-web
```

```typescript
import * as lwk from "@blockstream/lwk-web";
```

Node.js entrypoint:

```shell
$ npm install @blockstream/lwk-node
```

```typescript
import * as lwk from "@blockstream/lwk-node";
```

Or try it through the Liquid Web Wallet source:

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

### Build npm packages for release

Build the generated wasm bindings and both published npm package layouts.

```shell
$ cd lwk_wasm/npm
$ npm ci
$ npm run build
$ npm pack --dry-run --workspace @blockstream/lwk-web
$ npm pack --dry-run --workspace @blockstream/lwk-node
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

### Test Node.js examples

Requirement:

* having installed the npm dependencies inside `lwk_wasm/npm`
* having node and npm installed

```shell
cd lwk_wasm/npm
npm run test:node
```

## Javascript code conventions

For new additions and improvements, follow our [guidelines](GUIDE.md).
