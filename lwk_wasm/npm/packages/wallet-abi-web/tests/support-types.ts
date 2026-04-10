import {
  AssetId,
  ContractHash,
  LockTime,
  Network,
  OutPoint,
  PublicKey,
  Script,
  SecretKey,
  SimplicityArguments,
  SimplicityWitnessValues,
  TxSequence,
  Txid,
  XOnlyPublicKey,
} from "helpers_wallet_abi_web";
import {
  Network as NetworkFromSchema,
  Script as ScriptFromSchema,
} from "helpers_wallet_abi_web/schema";

const networkCtor: typeof NetworkFromSchema = Network;
const scriptCtor: typeof ScriptFromSchema = Script;

void networkCtor;
void scriptCtor;
void AssetId;
void ContractHash;
void LockTime;
void OutPoint;
void PublicKey;
void SecretKey;
void SimplicityArguments;
void SimplicityWitnessValues;
void TxSequence;
void Txid;
void XOnlyPublicKey;
