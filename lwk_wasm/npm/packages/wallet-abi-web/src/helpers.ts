import * as lwkWeb from "lwk_web";

type LwkWalletAbiWebModule = typeof import("lwk_web");
type LwkWalletAbiWebInitModule = LwkWalletAbiWebModule & {
  default?: (moduleOrPath?: unknown) => Promise<unknown>;
};

let initPromise: Promise<LwkWalletAbiWebModule> | undefined;

export function loadLwkWalletAbiWeb(
  moduleOrPath?: unknown
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
  pubkey: string
): lwkWeb.XOnlyPublicKey {
  return lwkWeb.XOnlyPublicKey.fromString(pubkey);
}

export function txidFromString(txid: string): lwkWeb.Txid {
  return new lwkWeb.Txid(txid);
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
