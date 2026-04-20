import {
  WalletAbiAmountFilter,
  WalletAbiAssetFilter,
  WalletAbiAssetVariant,
  WalletAbiBlinderVariant,
  WalletAbiInputIssuance,
  WalletAbiLockFilter,
  WalletAbiLockVariant,
  WalletAbiOutputSchema,
  WalletAbiRuntimeParams,
  WalletAbiWalletSourceFilter,
  createProcessRequest,
  createTxCreateRequest,
  createWalletInput,
  assetIdFromString,
  loadLwkWalletAbiWeb,
  networkFromString,
  walletAbiTxCreateRequestToJson,
  walletAbiNetworkToLwkNetworkName,
  type WalletAbiProcessRequest,
  type WalletAbiTxCreateRequest,
} from "lwk_wallet_abi_sdk";

import type { WalletAbiMethod } from "lwk_wallet_abi_sdk/protocol";

import {
  FIXTURE_REISSUANCE_TOKEN_ASSET,
  type IssuanceScenario,
  type ReissuanceScenario,
  type ScenarioRecipient,
  type ScenarioV1,
  type SplitScenario,
  type TransferScenario,
} from "./scenario.js";
import { parseRawEnvelope, type HarnessRawEnvelope } from "./url.js";

export interface HarnessRawSuccessEnvelope {
  id: number | string;
  jsonrpc: string;
  result: unknown;
}

const FIXTURE_ENTROPY_SEEDS = new Map<string, Uint8Array>([
  ["fixture:issuance", new Uint8Array(32).fill(7)],
  ["fixture:reissuance", new Uint8Array(32).fill(8)],
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeResult(value: unknown): unknown {
  if (typeof value !== "string") {
    return value;
  }

  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

function parsePositiveBigInt(value: string, label: string): bigint {
  if (!/^\d+$/u.test(value.trim())) {
    throw new Error(`${label} must be an unsigned integer string`);
  }

  return BigInt(value);
}

function parseFeeRateSatKvb(value: number | null): number | null {
  if (value === null) {
    return null;
  }

  if (!Number.isFinite(value) || value < 0) {
    throw new Error("Fee rate must be a non-negative number");
  }

  return value;
}

async function resolveEntropyBytes(entropySeed: string): Promise<Uint8Array> {
  const fixtureEntropy = FIXTURE_ENTROPY_SEEDS.get(entropySeed);
  if (fixtureEntropy !== undefined) {
    return fixtureEntropy;
  }

  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`wallet-abi-harness:${entropySeed}`),
  );
  return new Uint8Array(digest);
}

function resolveNetwork(scenario: ScenarioV1) {
  return networkFromString(walletAbiNetworkToLwkNetworkName(scenario.network));
}

function resolveAssetId(
  assetId: string,
  network: ReturnType<typeof resolveNetwork>,
) {
  const normalizedAssetId = assetId.trim();
  if (normalizedAssetId.length === 0) {
    return network.policyAsset();
  }

  return assetIdFromString(normalizedAssetId);
}

async function createRecipientOutput(
  recipient: ScenarioRecipient,
  scenario: TransferScenario | SplitScenario,
): Promise<WalletAbiOutputSchema> {
  const network = resolveNetwork(scenario);
  const lwk = await loadLwkWalletAbiWeb();
  const address = lwk.Address.parse(recipient.recipientAddress, network);

  return WalletAbiOutputSchema.new(
    recipient.id,
    parsePositiveBigInt(recipient.amountSat, `${recipient.id} amount`),
    WalletAbiLockVariant.script(address.scriptPubkey()),
    WalletAbiAssetVariant.assetId(resolveAssetId(recipient.assetId, network)),
    WalletAbiBlinderVariant.wallet(),
  );
}

async function createTransferRequest(
  scenario: TransferScenario,
): Promise<WalletAbiTxCreateRequest> {
  const network = resolveNetwork(scenario);
  const outputs = [await createRecipientOutput(scenario.recipient, scenario)];
  const params = WalletAbiRuntimeParams.new(
    [],
    outputs,
    parseFeeRateSatKvb(scenario.feeRateSatKvb),
    null,
  );

  return createTxCreateRequest({
    network,
    params,
    broadcast: scenario.broadcast,
  });
}

async function createSplitRequest(
  scenario: SplitScenario,
): Promise<WalletAbiTxCreateRequest> {
  const network = resolveNetwork(scenario);
  const outputs = await Promise.all(
    scenario.recipients.map((recipient) =>
      createRecipientOutput(recipient, scenario),
    ),
  );
  const params = WalletAbiRuntimeParams.new(
    [],
    outputs,
    parseFeeRateSatKvb(scenario.feeRateSatKvb),
    null,
  );

  return createTxCreateRequest({
    network,
    params,
    broadcast: scenario.broadcast,
  });
}

async function createIssuanceRequest(
  scenario: IssuanceScenario,
): Promise<WalletAbiTxCreateRequest> {
  const network = resolveNetwork(scenario);
  const assetAmountSat = parsePositiveBigInt(
    scenario.assetAmountSat,
    "Issued asset amount",
  );
  const tokenAmountSat = parsePositiveBigInt(
    scenario.tokenAmountSat,
    "Issued token amount",
  );
  const issuance = WalletAbiInputIssuance.new(
    assetAmountSat,
    tokenAmountSat,
    await resolveEntropyBytes(scenario.entropySeed),
  );
  const inputs = [
    createWalletInput({
      id: scenario.walletInputId,
      filter: WalletAbiWalletSourceFilter.any(),
      issuance,
    }),
  ];
  const outputs = [
    WalletAbiOutputSchema.new(
      "issued_asset",
      assetAmountSat,
      WalletAbiLockVariant.wallet(),
      WalletAbiAssetVariant.newIssuanceAsset(0),
      WalletAbiBlinderVariant.wallet(),
    ),
  ];

  if (tokenAmountSat > 0n) {
    outputs.push(
      WalletAbiOutputSchema.new(
        "reissuance_token",
        tokenAmountSat,
        WalletAbiLockVariant.wallet(),
        WalletAbiAssetVariant.newIssuanceToken(0),
        WalletAbiBlinderVariant.wallet(),
      ),
    );
  }

  const params = WalletAbiRuntimeParams.new(
    inputs,
    outputs,
    parseFeeRateSatKvb(scenario.feeRateSatKvb),
    null,
  );

  return createTxCreateRequest({
    network,
    params,
    broadcast: scenario.broadcast,
  });
}

async function createReissuanceRequest(
  scenario: ReissuanceScenario,
): Promise<WalletAbiTxCreateRequest> {
  const network = resolveNetwork(scenario);
  const assetAmountSat = parsePositiveBigInt(
    scenario.assetAmountSat,
    "Reissued asset amount",
  );
  const tokenAmountSat = parsePositiveBigInt(
    scenario.tokenAmountSat,
    "Reissued token amount",
  );
  const issuance = WalletAbiInputIssuance.reissue(
    assetAmountSat,
    tokenAmountSat,
    await resolveEntropyBytes(scenario.entropySeed),
  );
  const filter = WalletAbiWalletSourceFilter.withFilters(
    WalletAbiAssetFilter.exact(
      assetIdFromString(
        scenario.tokenAssetId.trim().length > 0
          ? scenario.tokenAssetId.trim()
          : FIXTURE_REISSUANCE_TOKEN_ASSET,
      ),
    ),
    WalletAbiAmountFilter.none(),
    WalletAbiLockFilter.none(),
  );
  const inputs = [
    createWalletInput({
      id: scenario.walletInputId,
      filter,
      issuance,
    }),
  ];
  const outputs = [
    WalletAbiOutputSchema.new(
      "reissued_asset",
      assetAmountSat,
      WalletAbiLockVariant.wallet(),
      WalletAbiAssetVariant.reIssuanceAsset(0),
      WalletAbiBlinderVariant.wallet(),
    ),
  ];
  const params = WalletAbiRuntimeParams.new(
    inputs,
    outputs,
    parseFeeRateSatKvb(scenario.feeRateSatKvb),
    null,
  );

  return createTxCreateRequest({
    network,
    params,
    broadcast: scenario.broadcast,
  });
}

export async function scenarioToTxCreateRequest(
  scenario: ScenarioV1,
): Promise<WalletAbiTxCreateRequest> {
  switch (scenario.kind) {
    case "transfer":
      return createTransferRequest(scenario);
    case "split":
      return createSplitRequest(scenario);
    case "issuance":
      return createIssuanceRequest(scenario);
    case "reissuance":
      return createReissuanceRequest(scenario);
  }
}

export async function scenarioToProcessRequestEnvelope(
  scenario: ScenarioV1,
  rpcId = 1,
): Promise<WalletAbiProcessRequest> {
  const request = await scenarioToTxCreateRequest(scenario);
  return createProcessRequest(rpcId, request);
}

export async function createScenarioBundle(
  scenario: ScenarioV1,
  rpcId = 1,
): Promise<{
  request: WalletAbiTxCreateRequest;
  envelope: WalletAbiProcessRequest;
  requestJson: string;
  envelopeJson: string;
}> {
  const request = await scenarioToTxCreateRequest(scenario);
  const envelope = createProcessRequest(rpcId, request);

  return {
    request,
    envelope,
    requestJson: formatJson(walletAbiTxCreateRequestToJson(request)),
    envelopeJson: formatJson(envelope),
  };
}

export function formatJson(value: unknown): string {
  return `${JSON.stringify(
    value,
    (_key, candidate) =>
      typeof candidate === "bigint" ? candidate.toString() : candidate,
    2,
  )}\n`;
}

export function parseRawEnvelopeJson(json: string): HarnessRawEnvelope {
  const parsed = JSON.parse(json) as unknown;
  const envelope = parseRawEnvelope(parsed);
  if (envelope === null) {
    throw new Error("Expected a Wallet ABI JSON-RPC request envelope");
  }

  return envelope;
}

export function createRawSuccessEnvelope(
  envelope: HarnessRawEnvelope,
  result: unknown,
): HarnessRawSuccessEnvelope {
  return {
    id: envelope.id,
    jsonrpc: envelope.jsonrpc,
    result: normalizeResult(result),
  };
}

export function isWalletAbiProcessEnvelope(
  envelope: HarnessRawEnvelope,
): envelope is HarnessRawEnvelope & {
  method: "wallet_abi_process_request";
} {
  return envelope.method === "wallet_abi_process_request";
}

export function isWalletAbiGetterMethod(method: WalletAbiMethod): boolean {
  return (
    method === "get_signer_receive_address" ||
    method === "get_raw_signing_x_only_pubkey"
  );
}

export function validateRawEnvelope(
  value: unknown,
): value is HarnessRawEnvelope {
  return parseRawEnvelope(value) !== null;
}

export function ensureRawEnvelope(value: unknown): HarnessRawEnvelope {
  if (!isRecord(value)) {
    throw new Error("Expected a Wallet ABI JSON-RPC object");
  }

  const envelope = parseRawEnvelope(value);
  if (envelope === null) {
    throw new Error("Unsupported Wallet ABI JSON-RPC method");
  }

  return envelope;
}
