import assert from "node:assert/strict";
import test from "node:test";

import * as sdk from "lwk_wallet_abi_sdk";
import * as helpers from "lwk_wallet_abi_sdk/helpers";
import * as schema from "lwk_wallet_abi_sdk/schema";

test("root re-exports schema surface", () => {
  assert.equal(sdk.WalletAbiTxCreateRequest, schema.WalletAbiTxCreateRequest);
  assert.equal(sdk.WalletAbiTxCreateResponse, schema.WalletAbiTxCreateResponse);
  assert.equal(sdk.WalletAbiRuntimeParams, schema.WalletAbiRuntimeParams);
});

test("root re-exports helper surface", () => {
  assert.equal(sdk.loadLwkWalletAbiWeb, helpers.loadLwkWalletAbiWeb);
  assert.equal(sdk.walletAbiJsonString, helpers.walletAbiJsonString);
  assert.equal(sdk.networkFromString, helpers.networkFromString);
});
