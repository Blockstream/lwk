import assert from "node:assert/strict";
import test from "node:test";

import {
  WALLET_ABI_WALLETCONNECT_CHAINS,
  WALLET_ABI_WALLETCONNECT_NAMESPACE,
  createWalletAbiCaipNetwork,
  isWalletAbiWalletConnectChain,
  walletAbiNetworkToWalletConnectChain,
  walletConnectChainToWalletAbiNetwork,
} from "lwk_wallet_abi_sdk/walletconnect";

test("walletconnect chain mapping", () => {
  assert.equal(WALLET_ABI_WALLETCONNECT_NAMESPACE, "walabi");
  assert.deepEqual(WALLET_ABI_WALLETCONNECT_CHAINS, [
    "walabi:liquid",
    "walabi:testnet-liquid",
    "walabi:localtest-liquid",
  ]);

  assert.equal(isWalletAbiWalletConnectChain("walabi:liquid"), true);
  assert.equal(isWalletAbiWalletConnectChain("walabi:testnet-liquid"), true);
  assert.equal(isWalletAbiWalletConnectChain("walabi:localtest-liquid"), true);
  assert.equal(isWalletAbiWalletConnectChain("eip155:1"), false);

  assert.equal(walletAbiNetworkToWalletConnectChain("liquid"), "walabi:liquid");
  assert.equal(
    walletAbiNetworkToWalletConnectChain("testnet-liquid"),
    "walabi:testnet-liquid"
  );
  assert.equal(
    walletAbiNetworkToWalletConnectChain("localtest-liquid"),
    "walabi:localtest-liquid"
  );

  assert.equal(walletConnectChainToWalletAbiNetwork("walabi:liquid"), "liquid");
  assert.equal(
    walletConnectChainToWalletAbiNetwork("walabi:testnet-liquid"),
    "testnet-liquid"
  );
  assert.equal(
    walletConnectChainToWalletAbiNetwork("walabi:localtest-liquid"),
    "localtest-liquid"
  );
});

test("walletconnect caip network shape", () => {
  const network = createWalletAbiCaipNetwork("testnet-liquid");

  assert.equal(network.id, "testnet-liquid");
  assert.equal(network.caipNetworkId, "walabi:testnet-liquid");
  assert.equal(network.chainNamespace, WALLET_ABI_WALLETCONNECT_NAMESPACE);
  assert.equal(network.name, "Liquid Testnet");
  assert.equal(network.testnet, true);
});
