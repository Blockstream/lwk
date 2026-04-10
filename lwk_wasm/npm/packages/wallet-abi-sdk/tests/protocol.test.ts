import assert from "node:assert/strict";
import test from "node:test";

import {
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  WALLET_ABI_JSON_RPC_VERSION,
  WALLET_ABI_METHODS,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
} from "lwk_wallet_abi_sdk/protocol";

test("wallet abi protocol constants", () => {
  assert.equal(WALLET_ABI_JSON_RPC_VERSION, "2.0");
  assert.deepEqual(WALLET_ABI_METHODS, [
    GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
  ]);
});
