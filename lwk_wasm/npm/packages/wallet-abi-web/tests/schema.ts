import {
  Network,
  OutPoint,
  Script,
  TxSequence,
  Txid,
  WalletAbiAssetVariant,
  WalletAbiBlinderVariant,
  WalletAbiFinalizerSpec,
  WalletAbiInputSchema,
  WalletAbiInputUnblinding,
  WalletAbiLockVariant,
  WalletAbiOutputSchema,
  WalletAbiPreviewAssetDelta,
  WalletAbiPreviewOutput,
  WalletAbiPreviewOutputKind,
  WalletAbiRequestPreview,
  WalletAbiRuntimeParams,
  WalletAbiTransactionInfo,
  WalletAbiTxCreateRequest,
  WalletAbiTxCreateResponse,
  WalletAbiTxEvaluateRequest,
  WalletAbiTxEvaluateResponse,
  WalletAbiUtxoSource,
  WalletAbiWalletSourceFilter,
} from "lwk_wallet_abi_web";
import {
  WalletAbiTxCreateResponse as WalletAbiTxCreateResponseFromSchema,
  WalletAbiTxEvaluateRequest as WalletAbiTxEvaluateRequestFromSchema,
} from "lwk_wallet_abi_web/schema";
import { networkFromString } from "lwk_wallet_abi_web/helpers";

const network = networkFromString("liquid-testnet");
const policyAsset = network.policyAsset();
const input = WalletAbiInputSchema.fromSequence(
  "input-0",
  WalletAbiUtxoSource.provided(
    new OutPoint(
      "0000000000000000000000000000000000000000000000000000000000000000:0"
    )
  ),
  WalletAbiInputUnblinding.explicit(),
  TxSequence.zero(),
  WalletAbiFinalizerSpec.wallet()
);
const output = WalletAbiOutputSchema.new(
  "output-0",
  1_000n,
  WalletAbiLockVariant.script(Script.empty()),
  WalletAbiAssetVariant.assetId(policyAsset),
  WalletAbiBlinderVariant.explicit()
);
const params = WalletAbiRuntimeParams.new([input], [output], 100.0, null);
const createRequest = WalletAbiTxCreateRequest.fromParts(
  "00000000-0000-4000-8000-000000000000",
  network,
  params,
  false
);
const evaluateRequest = WalletAbiTxEvaluateRequest.fromParts(
  "00000000-0000-4000-8000-000000000001",
  network,
  params
);
const preview = WalletAbiRequestPreview.new(
  [WalletAbiPreviewAssetDelta.new(policyAsset, -1_000n)],
  [
    WalletAbiPreviewOutput.new(
      WalletAbiPreviewOutputKind.External,
      policyAsset,
      1_000n,
      Script.empty()
    ),
  ],
  []
);
const transaction = WalletAbiTransactionInfo.new(
  "00",
  new Txid("0000000000000000000000000000000000000000000000000000000000000000")
);
const createResponse = WalletAbiTxCreateResponse.ok(
  createRequest.requestId(),
  network,
  transaction,
  JSON.stringify({ preview: JSON.parse(preview.toString()) })
);
const evaluateResponse = WalletAbiTxEvaluateResponse.ok(
  evaluateRequest,
  preview
);

const createResponseCtor: typeof WalletAbiTxCreateResponseFromSchema =
  WalletAbiTxCreateResponse;
const evaluateRequestCtor: typeof WalletAbiTxEvaluateRequestFromSchema =
  WalletAbiTxEvaluateRequest;
const parsedPreview: WalletAbiRequestPreview | undefined = createResponse.preview();
const walletSourceFilter = WalletAbiWalletSourceFilter.any();

void Network;
void createResponseCtor;
void evaluateRequestCtor;
void parsedPreview;
void evaluateResponse;
void walletSourceFilter;
