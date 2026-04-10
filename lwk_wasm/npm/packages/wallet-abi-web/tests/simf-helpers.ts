import {
  WalletAbiFinalizerSpec,
  WalletAbiInternalKeySource,
  WalletAbiRuntimeSimfValue,
  WalletAbiRuntimeSimfWitness,
  WalletAbiSimfArguments,
  WalletAbiSimfWitness,
  WalletAbiTaprootHandle,
} from "lwk_wallet_abi_web";
import {
  WalletAbiFinalizerSpec as WalletAbiFinalizerSpecFromSchema,
  WalletAbiSimfArguments as WalletAbiSimfArgumentsFromSchema,
} from "lwk_wallet_abi_web/schema";

const finalizerCtor: typeof WalletAbiFinalizerSpecFromSchema =
  WalletAbiFinalizerSpec;
const simfArgumentsCtor: typeof WalletAbiSimfArgumentsFromSchema =
  WalletAbiSimfArguments;

void finalizerCtor;
void simfArgumentsCtor;
void WalletAbiInternalKeySource;
void WalletAbiRuntimeSimfValue;
void WalletAbiRuntimeSimfWitness;
void WalletAbiSimfWitness;
void WalletAbiTaprootHandle;
