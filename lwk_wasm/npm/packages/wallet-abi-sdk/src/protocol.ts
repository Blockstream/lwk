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

export interface WalletAbiJsonRpcSuccessResponse<TResult> {
  id: number;
  jsonrpc: typeof WALLET_ABI_JSON_RPC_VERSION;
  result: TResult;
}

export interface WalletAbiJsonRpcErrorObject {
  code: number;
  message: string;
}

export interface WalletAbiJsonRpcErrorResponse {
  id: number;
  jsonrpc: typeof WALLET_ABI_JSON_RPC_VERSION;
  error: WalletAbiJsonRpcErrorObject;
}

export interface WalletAbiGetSignerReceiveAddressRequest {
  id: number;
  jsonrpc: typeof WALLET_ABI_JSON_RPC_VERSION;
  method: typeof GET_SIGNER_RECEIVE_ADDRESS_METHOD;
  params?: Record<string, never>;
}

export interface WalletAbiGetRawSigningXOnlyPubkeyRequest {
  id: number;
  jsonrpc: typeof WALLET_ABI_JSON_RPC_VERSION;
  method: typeof GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD;
  params?: Record<string, never>;
}

export type WalletAbiGetSignerReceiveAddressResponse =
  WalletAbiJsonRpcSuccessResponse<string>;
export type WalletAbiGetRawSigningXOnlyPubkeyResponse =
  WalletAbiJsonRpcSuccessResponse<string>;

export class WalletAbiProtocolError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WalletAbiProtocolError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isWalletAbiMethod(value: string): value is WalletAbiMethod {
  return WALLET_ABI_METHODS.includes(value as WalletAbiMethod);
}

export function isJsonRpcErrorResponse(
  value: unknown
): value is WalletAbiJsonRpcErrorResponse {
  return isRecord(value) && isRecord(value.error);
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

export function createGetSignerReceiveAddressRequest(
  id: number
): WalletAbiGetSignerReceiveAddressRequest {
  return {
    id,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    method: GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  };
}

export function createGetRawSigningXOnlyPubkeyRequest(
  id: number
): WalletAbiGetRawSigningXOnlyPubkeyRequest {
  return {
    id,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    method: GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  };
}

export function parseGetSignerReceiveAddressResponse(value: unknown): string {
  if (isJsonRpcErrorResponse(value)) {
    throw new WalletAbiProtocolError(
      `${GET_SIGNER_RECEIVE_ADDRESS_METHOD} failed: ${value.error.message}`
    );
  }

  if (!isRecord(value) || typeof value.result !== "string") {
    throw new WalletAbiProtocolError(
      `expected ${GET_SIGNER_RECEIVE_ADDRESS_METHOD} result`
    );
  }

  return value.result;
}

export function parseGetRawSigningXOnlyPubkeyResponse(value: unknown): string {
  if (isJsonRpcErrorResponse(value)) {
    throw new WalletAbiProtocolError(
      `${GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD} failed: ${value.error.message}`
    );
  }

  if (!isRecord(value) || typeof value.result !== "string") {
    throw new WalletAbiProtocolError(
      `expected ${GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD} result`
    );
  }

  return value.result;
}
