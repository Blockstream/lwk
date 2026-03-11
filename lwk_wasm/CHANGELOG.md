# Changelog

## Unreleased

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
