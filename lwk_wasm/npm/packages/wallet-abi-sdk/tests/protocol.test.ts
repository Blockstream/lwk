import assert from "node:assert/strict";
import test from "node:test";

import {
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  WALLET_ABI_JSON_RPC_VERSION,
  WALLET_ABI_METHODS,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
  isWalletAbiGetterMethod,
  isWalletAbiMethod,
  isWalletAbiProcessMethod,
} from "lwk_wallet_abi_sdk/protocol";

test("wallet abi protocol constants", () => {
  assert.equal(WALLET_ABI_JSON_RPC_VERSION, "2.0");
  assert.deepEqual(WALLET_ABI_METHODS, [
    GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD,
  ]);
});

test("wallet abi method guards", () => {
  assert.equal(isWalletAbiMethod(GET_SIGNER_RECEIVE_ADDRESS_METHOD), true);
  assert.equal(isWalletAbiMethod(GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD), true);
  assert.equal(isWalletAbiMethod(WALLET_ABI_PROCESS_REQUEST_METHOD), true);
  assert.equal(isWalletAbiMethod("wallet_abi_unknown"), false);

  assert.equal(
    isWalletAbiGetterMethod(GET_SIGNER_RECEIVE_ADDRESS_METHOD),
    true
  );
  assert.equal(
    isWalletAbiGetterMethod(GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD),
    true
  );
  assert.equal(
    isWalletAbiGetterMethod(WALLET_ABI_PROCESS_REQUEST_METHOD),
    false
  );

  assert.equal(isWalletAbiProcessMethod(WALLET_ABI_PROCESS_REQUEST_METHOD), true);
  assert.equal(isWalletAbiProcessMethod(GET_SIGNER_RECEIVE_ADDRESS_METHOD), false);
});
