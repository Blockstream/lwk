import {
  WalletAbiCapabilities,
  WalletAbiErrorInfo,
  WalletAbiStatus,
  WalletAbiTransactionInfo,
  WalletAbiTxCreateRequest,
  WalletAbiTxCreateResponse,
  WalletAbiTxEvaluateRequest,
  WalletAbiTxEvaluateResponse,
} from "lwk_wallet_abi_web";
import {
  WalletAbiCapabilities as WalletAbiCapabilitiesFromSchema,
  WalletAbiTxCreateRequest as WalletAbiTxCreateRequestFromSchema,
} from "lwk_wallet_abi_web/schema";

const capabilityCtor: typeof WalletAbiCapabilitiesFromSchema =
  WalletAbiCapabilities;
const createRequestCtor: typeof WalletAbiTxCreateRequestFromSchema =
  WalletAbiTxCreateRequest;
const okStatus: WalletAbiStatus = WalletAbiStatus.Ok;

void capabilityCtor;
void createRequestCtor;
void okStatus;
void WalletAbiErrorInfo;
void WalletAbiTransactionInfo;
void WalletAbiTxCreateResponse;
void WalletAbiTxEvaluateRequest;
void WalletAbiTxEvaluateResponse;
