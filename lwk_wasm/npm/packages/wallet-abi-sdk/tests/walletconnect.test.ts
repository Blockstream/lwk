import assert from "node:assert/strict";
import test from "node:test";

import {
  WALLET_ABI_WALLETCONNECT_CHAINS,
  WALLET_ABI_WALLETCONNECT_EVENTS,
  WALLET_ABI_WALLETCONNECT_METHODS,
  WALLET_ABI_WALLETCONNECT_NAMESPACE,
  createWalletAbiMetadata,
  createWalletAbiRequiredNamespaces,
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

test("walletconnect required namespaces", () => {
  assert.deepEqual(WALLET_ABI_WALLETCONNECT_METHODS, [
    "get_signer_receive_address",
    "get_raw_signing_x_only_pubkey",
    "wallet_abi_process_request",
  ]);
  assert.deepEqual(WALLET_ABI_WALLETCONNECT_EVENTS, []);
  assert.deepEqual(createWalletAbiRequiredNamespaces("testnet-liquid"), {
    walabi: {
      methods: WALLET_ABI_WALLETCONNECT_METHODS,
      chains: ["walabi:testnet-liquid"],
      events: WALLET_ABI_WALLETCONNECT_EVENTS,
    },
  });
  assert.deepEqual(createWalletAbiRequiredNamespaces("walabi:liquid"), {
    walabi: {
      methods: WALLET_ABI_WALLETCONNECT_METHODS,
      chains: ["walabi:liquid"],
      events: WALLET_ABI_WALLETCONNECT_EVENTS,
    },
  });
});

test("walletconnect metadata defaults and overrides", () => {
  const defaults = createWalletAbiMetadata("https://example.com/app");
  assert.equal(defaults.name, "LWK Wallet ABI SDK");
  assert.equal(
    defaults.description,
    "Wallet ABI WalletConnect session for a browser application."
  );
  assert.equal(defaults.url, "https://example.com/app");
  assert.deepEqual(defaults.icons, ["https://example.com/favicon.ico"]);
  assert.deepEqual(defaults.redirect, {
    universal: "https://example.com/app",
  });

  const overridden = createWalletAbiMetadata("https://example.com/app", {
    name: "Custom Wallet ABI",
    icons: ["https://cdn.example.com/icon.png"],
  });
  assert.equal(overridden.name, "Custom Wallet ABI");
  assert.deepEqual(overridden.icons, ["https://cdn.example.com/icon.png"]);
  assert.equal(overridden.url, "https://example.com/app");
});
