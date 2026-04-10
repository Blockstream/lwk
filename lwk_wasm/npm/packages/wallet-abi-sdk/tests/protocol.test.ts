import assert from "node:assert/strict";
import test from "node:test";

import { networkFromString } from "lwk_wallet_abi_web/helpers";
import {
  WalletAbiRuntimeParams,
  WalletAbiTransactionInfo,
  WalletAbiTxCreateRequest,
  WalletAbiTxCreateResponse,
  Txid,
} from "lwk_wallet_abi_web/schema";
import {
  createProcessRequest,
  createGetRawSigningXOnlyPubkeyRequest,
  createGetSignerReceiveAddressRequest,
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  LWK_WALLET_ABI_NETWORK_NAMES,
  WalletAbiProtocolError,
  WALLET_ABI_JSON_RPC_VERSION,
  WALLET_ABI_METHODS,
  WALLET_ABI_NETWORKS,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
  isWalletAbiGetterMethod,
  isJsonRpcErrorResponse,
  isWalletAbiMethod,
  isWalletAbiProcessMethod,
  parseProcessRequestResponse,
  parseGetRawSigningXOnlyPubkeyResponse,
  parseGetSignerReceiveAddressResponse,
  walletAbiNetworkFromLwkNetworkName,
  walletAbiNetworkFromNetwork,
  walletAbiNetworkToLwkNetworkName,
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

test("wallet abi network name translation", () => {
  assert.deepEqual(LWK_WALLET_ABI_NETWORK_NAMES, [
    "liquid",
    "liquid-testnet",
    "liquid-regtest",
  ]);
  assert.deepEqual(WALLET_ABI_NETWORKS, [
    "liquid",
    "testnet-liquid",
    "localtest-liquid",
  ]);

  assert.equal(walletAbiNetworkFromLwkNetworkName("liquid"), "liquid");
  assert.equal(
    walletAbiNetworkFromLwkNetworkName("liquid-testnet"),
    "testnet-liquid"
  );
  assert.equal(
    walletAbiNetworkFromLwkNetworkName("liquid-regtest"),
    "localtest-liquid"
  );

  assert.equal(walletAbiNetworkToLwkNetworkName("liquid"), "liquid");
  assert.equal(
    walletAbiNetworkToLwkNetworkName("testnet-liquid"),
    "liquid-testnet"
  );
  assert.equal(
    walletAbiNetworkToLwkNetworkName("localtest-liquid"),
    "liquid-regtest"
  );

  assert.equal(
    walletAbiNetworkFromNetwork(networkFromString("liquid-testnet")),
    "testnet-liquid"
  );
});

test("get signer receive address envelope", () => {
  assert.deepEqual(createGetSignerReceiveAddressRequest(7), {
    id: 7,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    method: GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  });

  assert.equal(
    parseGetSignerReceiveAddressResponse({
      id: 7,
      jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
      result: "tex1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq0qfhdv",
    }),
    "tex1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq0qfhdv"
  );

  assert.equal(
    isJsonRpcErrorResponse({
      id: 7,
      jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
      error: { code: -1, message: "boom" },
    }),
    true
  );

  assert.throws(
    () =>
      parseGetSignerReceiveAddressResponse({
        id: 7,
        jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
        error: { code: -1, message: "boom" },
      }),
    (error: unknown) =>
      error instanceof WalletAbiProtocolError &&
      error.message === `${GET_SIGNER_RECEIVE_ADDRESS_METHOD} failed: boom`
  );
});

test("get raw signing xonly pubkey envelope", () => {
  assert.deepEqual(createGetRawSigningXOnlyPubkeyRequest(8), {
    id: 8,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    method: GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  });

  assert.equal(
    parseGetRawSigningXOnlyPubkeyResponse({
      id: 8,
      jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
      result: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    }),
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
  );

  assert.throws(
    () =>
      parseGetRawSigningXOnlyPubkeyResponse({
        id: 8,
        jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
        error: { code: -1, message: "boom" },
      }),
    (error: unknown) =>
      error instanceof WalletAbiProtocolError &&
      error.message === `${GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD} failed: boom`
  );
});

test("process request envelope", () => {
  const network = networkFromString("liquid-testnet");
  const request = WalletAbiTxCreateRequest.fromParts(
    "00000000-0000-4000-8000-000000000002",
    network,
    WalletAbiRuntimeParams.new([], [], null, null),
    false
  );
  const response = WalletAbiTxCreateResponse.ok(
    request.requestId(),
    network,
    WalletAbiTransactionInfo.new(
      "00",
      new Txid("0000000000000000000000000000000000000000000000000000000000000000")
    )
  );

  assert.deepEqual(createProcessRequest(9, request), {
    id: 9,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    method: WALLET_ABI_PROCESS_REQUEST_METHOD,
    params: request.toJSON(),
  });

  const parsed = parseProcessRequestResponse({
    id: 9,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    result: response.toJSON(),
  });

  assert.equal(parsed.requestId(), request.requestId());
  assert.equal(parsed.status(), response.status());
});
