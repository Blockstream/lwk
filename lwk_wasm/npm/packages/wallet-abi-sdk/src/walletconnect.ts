import type { CustomCaipNetwork } from "@reown/appkit-common";

import {
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
  type WalletAbiMethod,
  type WalletAbiTransportNetwork,
} from "./protocol.js";

export const WALLET_ABI_WALLETCONNECT_NAMESPACE = "walabi" as const;

export const WALLET_ABI_WALLETCONNECT_CHAINS = [
  "walabi:liquid",
  "walabi:testnet-liquid",
  "walabi:localtest-liquid",
] as const;
export const WALLET_ABI_WALLETCONNECT_METHODS = [
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
] as const;
export const WALLET_ABI_WALLETCONNECT_EVENTS = [] as const;

export type WalletAbiWalletConnectChain =
  (typeof WALLET_ABI_WALLETCONNECT_CHAINS)[number];

export interface WalletAbiWalletConnectNamespace {
  methods: readonly WalletAbiMethod[];
  chains: readonly WalletAbiWalletConnectChain[];
  events: readonly string[];
  accounts?: readonly string[];
}

const WALLET_ABI_NATIVE_CURRENCY = {
  name: "Liquid Bitcoin",
  symbol: "L-BTC",
  decimals: 8,
} as const;

function walletAbiNetworkName(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "Liquid";
    case "testnet-liquid":
      return "Liquid Testnet";
    case "localtest-liquid":
      return "Liquid Localtest";
  }
}

function walletAbiNetworkRpcUrl(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "https://blockstream.info/liquid/api";
    case "testnet-liquid":
      return "https://blockstream.info/liquidtestnet/api";
    case "localtest-liquid":
      return "http://127.0.0.1:3001";
  }
}

function walletAbiNetworkExplorerUrl(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "https://blockstream.info/liquid";
    case "testnet-liquid":
      return "https://blockstream.info/liquidtestnet";
    case "localtest-liquid":
      return "http://127.0.0.1:3001";
  }
}

export function isWalletAbiWalletConnectChain(
  value: string
): value is WalletAbiWalletConnectChain {
  return WALLET_ABI_WALLETCONNECT_CHAINS.includes(
    value as WalletAbiWalletConnectChain
  );
}

export function walletAbiNetworkToWalletConnectChain(
  network: WalletAbiTransportNetwork
): WalletAbiWalletConnectChain {
  switch (network) {
    case "liquid":
      return "walabi:liquid";
    case "testnet-liquid":
      return "walabi:testnet-liquid";
    case "localtest-liquid":
      return "walabi:localtest-liquid";
  }
}

export function walletConnectChainToWalletAbiNetwork(
  chainId: WalletAbiWalletConnectChain
): WalletAbiTransportNetwork {
  switch (chainId) {
    case "walabi:liquid":
      return "liquid";
    case "walabi:testnet-liquid":
      return "testnet-liquid";
    case "walabi:localtest-liquid":
      return "localtest-liquid";
  }
}

export function createWalletAbiCaipNetwork(
  network: WalletAbiTransportNetwork
): CustomCaipNetwork {
  const caipNetworkId = walletAbiNetworkToWalletConnectChain(network);
  const [, chainReference] = caipNetworkId.split(":");
  const name = walletAbiNetworkName(network);
  const rpcUrl = walletAbiNetworkRpcUrl(network);

  return {
    id: chainReference,
    name,
    caipNetworkId,
    chainNamespace: WALLET_ABI_WALLETCONNECT_NAMESPACE,
    nativeCurrency: WALLET_ABI_NATIVE_CURRENCY,
    rpcUrls: {
      default: {
        http: [rpcUrl],
      },
      public: {
        http: [rpcUrl],
      },
    },
    blockExplorers: {
      default: {
        name: `${name} Explorer`,
        url: walletAbiNetworkExplorerUrl(network),
      },
    },
    testnet: network !== "liquid",
  } as unknown as CustomCaipNetwork;
}

export function createWalletAbiRequiredNamespaces(
  input:
    | WalletAbiTransportNetwork
    | WalletAbiWalletConnectChain
    | readonly WalletAbiWalletConnectChain[]
): Record<
  typeof WALLET_ABI_WALLETCONNECT_NAMESPACE,
  WalletAbiWalletConnectNamespace
> {
  let chains: readonly WalletAbiWalletConnectChain[];

  if (Array.isArray(input)) {
    chains = input;
  } else {
    const singleInput = input as
      | WalletAbiTransportNetwork
      | WalletAbiWalletConnectChain;

    if (isWalletAbiWalletConnectChain(singleInput)) {
      chains = [singleInput];
    } else {
      chains = [walletAbiNetworkToWalletConnectChain(singleInput)];
    }
  }

  return {
    [WALLET_ABI_WALLETCONNECT_NAMESPACE]: {
      methods: WALLET_ABI_WALLETCONNECT_METHODS,
      chains,
      events: WALLET_ABI_WALLETCONNECT_EVENTS,
    },
  };
}
