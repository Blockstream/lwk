import assert from "node:assert/strict";
import test from "node:test";

import * as sdk from "lwk_wallet_abi_sdk";
import * as schema from "lwk_wallet_abi_sdk/schema";

test("root re-exports schema surface", () => {
  assert.equal(sdk.WalletAbiTxCreateRequest, schema.WalletAbiTxCreateRequest);
  assert.equal(sdk.WalletAbiTxCreateResponse, schema.WalletAbiTxCreateResponse);
  assert.equal(sdk.WalletAbiRuntimeParams, schema.WalletAbiRuntimeParams);
});
