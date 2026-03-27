# Changelog

## Unreleased

* Async constructors for `Jade`, `JadeWebSocket`, and `Amp0Connected` where deprecated due to [wasm-bindgen/wasm-bindgen#4402](https://github.com/wasm-bindgen/wasm-bindgen/pull/4402)
  * To avoid generating invalid TS code, macros `skip_typescript` was added to all of them.
  * Created static async factories `Jade.fromSerial(...)`, `JadeWebSocket.fromWebSocket(...)`, and `Amp0Connected.connect(...)` to replace deprecated constructors

## 0.16.0

* `TxOutSecrets` no longer exposes the asset blinding factor as a plain hex string in the old wasm API shape.
  * Use `assetBlindingFactor()` to get an `AssetBlindingFactor`.
  * Then call `toString()` if you need the hex form, or `toBytes()` if you need raw bytes.
* The same migration applies to `valueBlindingFactor()` and `ValueBlindingFactor`.

### Asset and Value blinding factors migration
Before:
```javascript
const assetBfHex = txOutSecrets.assetBlindingFactor();
```
After:
```javascript
const assetBfHex = txOutSecrets.assetBlindingFactor().toString();
```
