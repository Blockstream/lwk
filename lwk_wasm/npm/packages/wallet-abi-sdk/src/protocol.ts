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

export type WalletAbiMethod = (typeof WALLET_ABI_METHODS)[number];

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
