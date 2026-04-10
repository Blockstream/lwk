import type { Network } from "lwk_wallet_abi_web/schema";

export const WALLET_ABI_JSON_RPC_VERSION = "2.0" as const;

export const GET_SIGNER_RECEIVE_ADDRESS_METHOD =
  "get_signer_receive_address" as const;
export const GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD =
  "get_raw_signing_x_only_pubkey" as const;
export const WALLET_ABI_PROCESS_REQUEST_METHOD =
  "wallet_abi_process_request" as const;

export const WALLET_ABI_METHODS = [
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
] as const;

export const LWK_WALLET_ABI_NETWORK_NAMES = [
  "liquid",
  "liquid-testnet",
  "liquid-regtest",
] as const;

export const WALLET_ABI_NETWORKS = [
  "liquid",
  "testnet-liquid",
  "localtest-liquid",
] as const;

export type WalletAbiMethod = (typeof WALLET_ABI_METHODS)[number];
export type LwkWalletAbiNetworkName =
  (typeof LWK_WALLET_ABI_NETWORK_NAMES)[number];
export type WalletAbiTransportNetwork = (typeof WALLET_ABI_NETWORKS)[number];

export function isWalletAbiMethod(value: string): value is WalletAbiMethod {
  return WALLET_ABI_METHODS.includes(value as WalletAbiMethod);
}

export function isWalletAbiGetterMethod(
  value: string
): value is
  | typeof GET_SIGNER_RECEIVE_ADDRESS_METHOD
  | typeof GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD {
  return (
    value === GET_SIGNER_RECEIVE_ADDRESS_METHOD ||
    value === GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD
  );
}

export function isWalletAbiProcessMethod(
  value: string
): value is typeof WALLET_ABI_PROCESS_REQUEST_METHOD {
  return value === WALLET_ABI_PROCESS_REQUEST_METHOD;
}

export function walletAbiNetworkFromLwkNetworkName(
  value: LwkWalletAbiNetworkName
): WalletAbiTransportNetwork {
  switch (value) {
    case "liquid":
      return "liquid";
    case "liquid-testnet":
      return "testnet-liquid";
    case "liquid-regtest":
      return "localtest-liquid";
  }
}

export function walletAbiNetworkToLwkNetworkName(
  value: WalletAbiTransportNetwork
): LwkWalletAbiNetworkName {
  switch (value) {
    case "liquid":
      return "liquid";
    case "testnet-liquid":
      return "liquid-testnet";
    case "localtest-liquid":
      return "liquid-regtest";
  }
}

export function walletAbiNetworkFromNetwork(
  network: Network
): WalletAbiTransportNetwork {
  if (network.isMainnet()) {
    return "liquid";
  }

  if (network.isTestnet()) {
    return "testnet-liquid";
  }

  if (network.isRegtest()) {
    return "localtest-liquid";
  }

  throw new Error(`unsupported wallet network "${network.toString()}"`);
}
