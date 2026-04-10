import * as lwkWeb from "wallet_abi_sdk_core_web";

type LwkWalletAbiWebModule = typeof import("wallet_abi_sdk_core_web");
type LwkWalletAbiWebInitModule = LwkWalletAbiWebModule & {
  default?: (moduleOrPath?: unknown) => Promise<unknown>;
};

let initPromise: Promise<LwkWalletAbiWebModule> | undefined;

export function loadLwkWalletAbiWeb(
  moduleOrPath?: unknown,
): Promise<LwkWalletAbiWebModule> {
  if (initPromise) {
    return initPromise;
  }

  const module = lwkWeb as LwkWalletAbiWebInitModule;
  const promise = (async (): Promise<LwkWalletAbiWebModule> => {
    if (typeof module.default === "function") {
      await module.default(moduleOrPath);
    }

    return lwkWeb;
  })();

  initPromise = promise.catch((error: unknown) => {
    initPromise = undefined;
    throw error;
  });

  return initPromise;
}

export function assetIdFromString(assetId: string): lwkWeb.AssetId {
  return lwkWeb.AssetId.fromString(assetId);
}

export function outPointFromString(outPoint: string): lwkWeb.OutPoint {
  return new lwkWeb.OutPoint(outPoint);
}

export function scriptFromHex(scriptHex: string): lwkWeb.Script {
  return new lwkWeb.Script(scriptHex);
}

export function contractHashFromString(hash: string): lwkWeb.ContractHash {
  return lwkWeb.ContractHash.fromString(hash);
}

export function publicKeyFromString(pubkey: string): lwkWeb.PublicKey {
  return lwkWeb.PublicKey.fromString(pubkey);
}

export function xOnlyPublicKeyFromString(
  pubkey: string,
): lwkWeb.XOnlyPublicKey {
  return lwkWeb.XOnlyPublicKey.fromString(pubkey);
}

export function txidFromString(txid: string): lwkWeb.Txid {
  return new lwkWeb.Txid(txid);
}

export function networkFromString(
  name: string,
  policyAssetHex?: string,
): lwkWeb.Network {
  switch (name) {
    case "liquid":
      return lwkWeb.Network.mainnet();
    case "liquid-testnet":
      return lwkWeb.Network.testnet();
    case "liquid-regtest":
      return policyAssetHex === undefined
        ? lwkWeb.Network.regtestDefault()
        : lwkWeb.Network.regtest(assetIdFromString(policyAssetHex));
    default:
      throw new Error(`Unsupported Liquid network: ${name}`);
  }
}

type WalletAbiJsonValue =
  | lwkWeb.WalletAbiCapabilities
  | lwkWeb.WalletAbiErrorInfo
  | lwkWeb.WalletAbiRequestPreview
  | lwkWeb.WalletAbiRuntimeParams
  | lwkWeb.WalletAbiTxCreateRequest
  | lwkWeb.WalletAbiTxCreateResponse
  | lwkWeb.WalletAbiTxEvaluateRequest
  | lwkWeb.WalletAbiTxEvaluateResponse;

export function walletAbiJsonString(value: WalletAbiJsonValue): string {
  return value.toString();
}
