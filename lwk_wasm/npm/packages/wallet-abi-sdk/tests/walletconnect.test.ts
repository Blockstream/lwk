import assert from "node:assert/strict";
import test from "node:test";

import type { SessionTypes, SignClientTypes } from "@walletconnect/types";
import {
  WalletAbiRuntimeParams,
  WalletAbiTxCreateRequest,
} from "lwk_wallet_abi_sdk/schema";
import { networkFromString } from "lwk_wallet_abi_sdk/helpers";
import {
  createGetSignerReceiveAddressRequest,
  createProcessRequest,
} from "lwk_wallet_abi_sdk/protocol";
import {
  WALLET_ABI_WALLETCONNECT_CHAINS,
  WALLET_ABI_WALLETCONNECT_EVENTS,
  WALLET_ABI_WALLETCONNECT_METHODS,
  WALLET_ABI_WALLETCONNECT_NAMESPACE,
  awaitWalletAbiApprovedSession,
  createWalletAbiMetadata,
  createWalletAbiRequiredNamespaces,
  createWalletAbiSessionController,
  createWalletConnectRequester,
  createWalletAbiCaipNetwork,
  isWalletAbiSession,
  isWalletAbiWalletConnectChain,
  selectWalletAbiSessions,
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
    "walabi:testnet-liquid",
  );
  assert.equal(
    walletAbiNetworkToWalletConnectChain("localtest-liquid"),
    "walabi:localtest-liquid",
  );

  assert.equal(walletConnectChainToWalletAbiNetwork("walabi:liquid"), "liquid");
  assert.equal(
    walletConnectChainToWalletAbiNetwork("walabi:testnet-liquid"),
    "testnet-liquid",
  );
  assert.equal(
    walletConnectChainToWalletAbiNetwork("walabi:localtest-liquid"),
    "localtest-liquid",
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
    "Wallet ABI WalletConnect session for a browser application.",
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

test("walletconnect requester normalizes getter params", async () => {
  const calls: unknown[] = [];
  const requester = createWalletConnectRequester({
    chainId: "walabi:testnet-liquid",
    client: {
      request(input) {
        calls.push(input);
        return "tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqf6u0sd";
      },
    },
  });

  const response = await requester.request(
    createGetSignerReceiveAddressRequest(5),
  );

  assert.deepEqual(calls, [
    {
      chainId: "walabi:testnet-liquid",
      request: {
        method: "get_signer_receive_address",
        params: {},
      },
    },
  ]);
  assert.deepEqual(response, {
    id: 5,
    jsonrpc: "2.0",
    result: "tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqf6u0sd",
  });
});

test("walletconnect requester forwards tx-create params and dynamic topic", async () => {
  const calls: unknown[] = [];
  const requester = createWalletConnectRequester({
    chainId: "walabi:testnet-liquid",
    topic: "ignored",
    getTopic() {
      return "topic-1";
    },
    client: {
      request(input) {
        calls.push(input);
        return { status: "ok" };
      },
    },
  });

  const request = WalletAbiTxCreateRequest.fromParts(
    "00000000-0000-4000-8000-000000000003",
    networkFromString("liquid-testnet"),
    WalletAbiRuntimeParams.new([], [], null, null),
    false,
  );
  const response = await requester.request(createProcessRequest(6, request));

  assert.deepEqual(calls, [
    {
      chainId: "walabi:testnet-liquid",
      topic: "topic-1",
      request: {
        method: "wallet_abi_process_request",
        params: {
          ...request.toJSON(),
          network: "testnet-liquid",
        },
      },
    },
  ]);
  assert.deepEqual(response, {
    id: 6,
    jsonrpc: "2.0",
    result: { status: "ok" },
  });
});

test("walletconnect session selection prefers newest active session", () => {
  const chainId = "walabi:testnet-liquid";
  const newest = {
    topic: "topic-new",
    expiry: 20,
    namespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
        accounts: [`${chainId}:wallet`],
      },
    },
    requiredNamespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
      },
    },
  } as unknown as SessionTypes.Struct;
  const oldest = {
    ...newest,
    topic: "topic-old",
    expiry: 10,
  } as SessionTypes.Struct;
  const otherChain = {
    ...newest,
    topic: "topic-mainnet",
    expiry: 30,
    namespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: ["walabi:liquid"],
        events: [],
        accounts: ["walabi:liquid:wallet"],
      },
    },
    requiredNamespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: ["walabi:liquid"],
        events: [],
      },
    },
  } as SessionTypes.Struct;

  assert.equal(isWalletAbiSession(newest, chainId), true);
  assert.equal(isWalletAbiSession(otherChain, chainId), false);

  const selected = selectWalletAbiSessions(
    [oldest, newest, otherChain],
    chainId,
  );

  assert.equal(selected.activeSession?.topic, "topic-new");
  assert.deepEqual(
    selected.staleSessions.map((session) => session.topic),
    ["topic-old"],
  );
});

test("walletconnect approval wait reuses existing session", async () => {
  const chainId = "walabi:testnet-liquid";
  const session = {
    topic: "topic-existing",
    expiry: 10,
    namespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
        accounts: [`${chainId}:wallet`],
      },
    },
    requiredNamespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
      },
    },
  } as unknown as SessionTypes.Struct;

  const selected = await awaitWalletAbiApprovedSession({
    approval() {
      throw new Error("should not run approval");
    },
    signClient: {
      session: {
        getAll() {
          return [session];
        },
      },
      on() {},
      off() {},
    },
    chainId,
  });

  assert.equal(selected.topic, "topic-existing");
});

test("walletconnect approval wait falls back to connected session", async () => {
  const chainId = "walabi:testnet-liquid";
  const listeners = new Set<
    (event: SignClientTypes.EventArguments["session_connect"]) => void
  >();
  const sessions: SessionTypes.Struct[] = [];
  const session = {
    topic: "topic-approved",
    expiry: 10,
    namespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
        accounts: [`${chainId}:wallet`],
      },
    },
    requiredNamespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
      },
    },
  } as unknown as SessionTypes.Struct;

  const result = await awaitWalletAbiApprovedSession({
    approval() {
      sessions.push(session);
      return Promise.reject(new Error("approval rejected"));
    },
    signClient: {
      session: {
        getAll() {
          return [...sessions];
        },
      },
      on(_event, listener) {
        listeners.add(listener);
      },
      off(_event, listener) {
        listeners.delete(listener);
      },
    },
    chainId,
    approvalRejectionGraceMs: 50,
    sessionPollMs: 5,
  });

  assert.equal(result.topic, "topic-approved");
  assert.equal(listeners.size, 0);
});

test("walletconnect approval wait polls while approval is pending", async () => {
  const chainId = "walabi:testnet-liquid";
  const session = {
    topic: "topic-polled",
    expiry: 10,
    namespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
        accounts: [`${chainId}:wallet`],
      },
    },
    requiredNamespaces: {
      walabi: {
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        chains: [chainId],
        events: [],
      },
    },
  } as unknown as SessionTypes.Struct;
  let polls = 0;

  const result = await awaitWalletAbiApprovedSession({
    approval() {
      return new Promise<SessionTypes.Struct>(() => {});
    },
    signClient: {
      session: {
        getAll() {
          polls += 1;
          return polls >= 2 ? [session] : [];
        },
      },
      on() {},
      off() {},
    },
    chainId,
    connectTimeoutMs: 100,
    sessionPollMs: 5,
  });

  assert.equal(result.topic, "topic-polled");
});

test("walletconnect session controller factory is exported", () => {
  assert.equal(typeof createWalletAbiSessionController, "function");
});
