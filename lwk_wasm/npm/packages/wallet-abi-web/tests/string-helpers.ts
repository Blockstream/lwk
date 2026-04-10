import {
  AssetId,
  ContractHash,
  Network,
  OutPoint,
  PublicKey,
  Script,
  Txid,
  WalletAbiCapabilities,
  XOnlyPublicKey,
  assetIdFromString,
  contractHashFromString,
  outPointFromString,
  publicKeyFromString,
  scriptFromHex,
  txidFromString,
  walletAbiJsonString,
  xOnlyPublicKeyFromString,
} from "lwk_wallet_abi_web";
import {
  assetIdFromString as assetIdFromHelpers,
  walletAbiJsonString as walletAbiJsonStringFromHelpers,
} from "lwk_wallet_abi_web/helpers";

const asset: AssetId = assetIdFromString(
  "144c654344aa8b152b2573d7e26e4fbe0f2dba706f740dab9b257576c0eb0ef0"
);
const assetFromSubpath: AssetId = assetIdFromHelpers(
  "144c654344aa8b152b2573d7e26e4fbe0f2dba706f740dab9b257576c0eb0ef0"
);
const outpoint: OutPoint = outPointFromString(
  "0000000000000000000000000000000000000000000000000000000000000000:0"
);
const script: Script = scriptFromHex("");
const contractHash: ContractHash = contractHashFromString(
  "0000000000000000000000000000000000000000000000000000000000000000"
);
const pubkey: PublicKey = publicKeyFromString(
  "02c6047f9441ed7d6d3045406e95c07cd85a11be33b4f598a5f8f64d2f2c0f3b8b"
);
const xonly: XOnlyPublicKey = xOnlyPublicKeyFromString(
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
);
const txid: Txid = txidFromString(
  "0000000000000000000000000000000000000000000000000000000000000000"
);
const capabilities = WalletAbiCapabilities.new(Network.testnet(), [
  "wallet_abi_process_request",
]);
const json: string = walletAbiJsonString(capabilities);
const jsonFromSubpath: string = walletAbiJsonStringFromHelpers(capabilities);

void asset;
void assetFromSubpath;
void outpoint;
void script;
void contractHash;
void pubkey;
void xonly;
void txid;
void json;
void jsonFromSubpath;
