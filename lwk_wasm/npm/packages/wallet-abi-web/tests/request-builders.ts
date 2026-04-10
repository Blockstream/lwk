import {
  WalletAbiAmountFilter,
  WalletAbiAssetFilter,
  WalletAbiAssetVariant,
  WalletAbiBlinderVariant,
  WalletAbiInputIssuance,
  WalletAbiInputIssuanceKind,
  WalletAbiInputSchema,
  WalletAbiInputUnblinding,
  WalletAbiLockFilter,
  WalletAbiLockVariant,
  WalletAbiOutputSchema,
  WalletAbiRuntimeParams,
  WalletAbiUtxoSource,
  WalletAbiWalletSourceFilter,
} from "lwk_wallet_abi_web";
import {
  WalletAbiOutputSchema as WalletAbiOutputSchemaFromSchema,
  WalletAbiRuntimeParams as WalletAbiRuntimeParamsFromSchema,
} from "lwk_wallet_abi_web/schema";

const runtimeCtor: typeof WalletAbiRuntimeParamsFromSchema =
  WalletAbiRuntimeParams;
const outputCtor: typeof WalletAbiOutputSchemaFromSchema = WalletAbiOutputSchema;
const issuanceKind: WalletAbiInputIssuanceKind = WalletAbiInputIssuanceKind.New;

void runtimeCtor;
void outputCtor;
void issuanceKind;
void WalletAbiAmountFilter;
void WalletAbiAssetFilter;
void WalletAbiAssetVariant;
void WalletAbiBlinderVariant;
void WalletAbiInputIssuance;
void WalletAbiInputSchema;
void WalletAbiInputUnblinding;
void WalletAbiLockFilter;
void WalletAbiLockVariant;
void WalletAbiUtxoSource;
void WalletAbiWalletSourceFilter;
