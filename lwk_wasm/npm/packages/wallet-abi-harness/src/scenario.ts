import type { WalletAbiTransportNetwork } from "lwk_wallet_abi_sdk/protocol";

export const TESTNET_POLICY_ASSET =
  "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
export const FIXTURE_REISSUANCE_TOKEN_ASSET =
  "38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5";
export const DEFAULT_TRANSFER_ADDRESS =
  "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
export const DEFAULT_SPLIT_ADDRESS =
  "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";

export const SCENARIO_KINDS = [
  "transfer",
  "split",
  "issuance",
  "reissuance",
] as const;
export const HARNESS_MODES = ["builder", "walletconnect"] as const;
export const TRANSPORT_NETWORKS = [
  "liquid",
  "testnet-liquid",
  "localtest-liquid",
] as const;

export type ScenarioKind = (typeof SCENARIO_KINDS)[number];
export type HarnessMode = (typeof HARNESS_MODES)[number];
export type ScenarioByKind<TKind extends ScenarioKind> = Extract<
  ScenarioV1,
  { kind: TKind }
>;

export interface ScenarioRecipient {
  id: string;
  recipientAddress: string;
  amountSat: string;
  assetId: string;
}

interface ScenarioBase {
  v: 1;
  mode: HarnessMode;
  kind: ScenarioKind;
  network: WalletAbiTransportNetwork;
  broadcast: boolean;
  feeRateSatKvb: number | null;
  autoConnect: boolean;
  autoSend: boolean;
}

export interface TransferScenario extends ScenarioBase {
  kind: "transfer";
  recipient: ScenarioRecipient;
}

export interface SplitScenario extends ScenarioBase {
  kind: "split";
  recipients: ScenarioRecipient[];
}

export interface IssuanceScenario extends ScenarioBase {
  kind: "issuance";
  walletInputId: string;
  assetAmountSat: string;
  tokenAmountSat: string;
  entropySeed: string;
}

export interface ReissuanceScenario extends ScenarioBase {
  kind: "reissuance";
  walletInputId: string;
  tokenAssetId: string;
  assetAmountSat: string;
  tokenAmountSat: string;
  entropySeed: string;
}

export type ScenarioV1 =
  | TransferScenario
  | SplitScenario
  | IssuanceScenario
  | ReissuanceScenario;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isTransportNetwork(
  value: unknown,
): value is WalletAbiTransportNetwork {
  return TRANSPORT_NETWORKS.includes(value as WalletAbiTransportNetwork);
}

export function isHarnessMode(value: unknown): value is HarnessMode {
  return HARNESS_MODES.includes(value as HarnessMode);
}

export function isScenarioKind(value: unknown): value is ScenarioKind {
  return SCENARIO_KINDS.includes(value as ScenarioKind);
}

function normalizeString(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim().length > 0
    ? value.trim()
    : fallback;
}

function normalizeNumberOrNull(
  value: unknown,
  fallback: number | null,
): number | null {
  if (value === null) {
    return null;
  }

  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return fallback;
  }

  return value;
}

function normalizeBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function normalizeRecipient(
  value: unknown,
  fallback: ScenarioRecipient,
  index = 0,
): ScenarioRecipient {
  if (!isRecord(value)) {
    return fallback;
  }

  return {
    id: normalizeString(value.id, `output-${String(index + 1)}`),
    recipientAddress: normalizeString(
      value.recipientAddress,
      fallback.recipientAddress,
    ),
    amountSat: normalizeString(value.amountSat, fallback.amountSat),
    assetId:
      typeof value.assetId === "string"
        ? value.assetId.trim()
        : fallback.assetId,
  };
}

function defaultAssetIdForNetwork(network: WalletAbiTransportNetwork): string {
  if (network === "testnet-liquid") {
    return TESTNET_POLICY_ASSET;
  }

  return "";
}

export function createRecipient(
  index: number,
  overrides: Partial<ScenarioRecipient> = {},
): ScenarioRecipient {
  return {
    id: overrides.id ?? `output-${String(index + 1)}`,
    recipientAddress:
      overrides.recipientAddress ??
      (index === 0 ? DEFAULT_TRANSFER_ADDRESS : DEFAULT_SPLIT_ADDRESS),
    amountSat: overrides.amountSat ?? (index === 0 ? "700" : "900"),
    assetId: overrides.assetId ?? TESTNET_POLICY_ASSET,
  };
}

export function createDefaultScenario<TKind extends ScenarioKind>(
  kind: TKind,
): ScenarioByKind<TKind>;
export function createDefaultScenario(): TransferScenario;
export function createDefaultScenario(
  kind: ScenarioKind = "transfer",
): ScenarioV1 {
  const base = {
    v: 1 as const,
    mode: "builder" as const,
    network: "testnet-liquid" as const,
    broadcast: true,
    feeRateSatKvb: 1000,
    autoConnect: false,
    autoSend: false,
  };

  switch (kind) {
    case "transfer":
      return {
        ...base,
        kind,
        recipient: createRecipient(0),
      };
    case "split":
      return {
        ...base,
        kind,
        recipients: [
          createRecipient(0),
          createRecipient(1, {
            recipientAddress: DEFAULT_SPLIT_ADDRESS,
          }),
        ],
      };
    case "issuance":
      return {
        ...base,
        kind,
        walletInputId: "issuance",
        assetAmountSat: "5",
        tokenAmountSat: "1",
        entropySeed: "fixture:issuance",
      };
    case "reissuance":
      return {
        ...base,
        kind,
        walletInputId: "reissuance",
        tokenAssetId: FIXTURE_REISSUANCE_TOKEN_ASSET,
        assetAmountSat: "7",
        tokenAmountSat: "0",
        entropySeed: "fixture:reissuance",
      };
  }
}

export function normalizeScenario(value: unknown): ScenarioV1 {
  if (!isRecord(value)) {
    return createDefaultScenario();
  }

  const kind = isScenarioKind(value.kind) ? value.kind : "transfer";
  const fallback = createDefaultScenario(kind);
  const mode = isHarnessMode(value.mode) ? value.mode : fallback.mode;
  const network = isTransportNetwork(value.network)
    ? value.network
    : fallback.network;
  const base = {
    v: 1 as const,
    mode,
    kind,
    network,
    broadcast: normalizeBoolean(value.broadcast, fallback.broadcast),
    feeRateSatKvb: normalizeNumberOrNull(
      value.feeRateSatKvb,
      fallback.feeRateSatKvb,
    ),
    autoConnect: normalizeBoolean(value.autoConnect, fallback.autoConnect),
    autoSend: normalizeBoolean(value.autoSend, fallback.autoSend),
  };

  switch (kind) {
    case "transfer": {
      const transferFallback = fallback as TransferScenario;
      return {
        ...base,
        kind,
        recipient: normalizeRecipient(
          value.recipient,
          transferFallback.recipient,
        ),
      };
    }
    case "split": {
      const splitFallback = fallback as SplitScenario;
      const recipients = Array.isArray(value.recipients)
        ? value.recipients.map((recipient, index) =>
            normalizeRecipient(recipient, createRecipient(index), index),
          )
        : splitFallback.recipients;

      return {
        ...base,
        kind,
        recipients:
          recipients.length >= 2 ? recipients : splitFallback.recipients,
      };
    }
    case "issuance": {
      const issuanceFallback = fallback as IssuanceScenario;
      return {
        ...base,
        kind,
        walletInputId: normalizeString(
          value.walletInputId,
          issuanceFallback.walletInputId,
        ),
        assetAmountSat: normalizeString(
          value.assetAmountSat,
          issuanceFallback.assetAmountSat,
        ),
        tokenAmountSat: normalizeString(
          value.tokenAmountSat,
          issuanceFallback.tokenAmountSat,
        ),
        entropySeed: normalizeString(
          value.entropySeed,
          issuanceFallback.entropySeed,
        ),
      };
    }
    case "reissuance": {
      const reissuanceFallback = fallback as ReissuanceScenario;
      return {
        ...base,
        kind,
        walletInputId: normalizeString(
          value.walletInputId,
          reissuanceFallback.walletInputId,
        ),
        tokenAssetId: normalizeString(
          value.tokenAssetId,
          reissuanceFallback.tokenAssetId,
        ),
        assetAmountSat: normalizeString(
          value.assetAmountSat,
          reissuanceFallback.assetAmountSat,
        ),
        tokenAmountSat: normalizeString(
          value.tokenAmountSat,
          reissuanceFallback.tokenAmountSat,
        ),
        entropySeed: normalizeString(
          value.entropySeed,
          reissuanceFallback.entropySeed,
        ),
      };
    }
  }
}

export function setScenarioKind(
  scenario: ScenarioV1,
  kind: ScenarioKind,
): ScenarioV1 {
  const nextScenario = createDefaultScenario(kind);
  return normalizeScenario({
    ...nextScenario,
    mode: scenario.mode,
    network: scenario.network,
    broadcast: scenario.broadcast,
    feeRateSatKvb: scenario.feeRateSatKvb,
    autoConnect: scenario.autoConnect,
    autoSend: scenario.autoSend,
  });
}

export function setScenarioMode(
  scenario: ScenarioV1,
  mode: HarnessMode,
): typeof scenario {
  return normalizeScenario({
    ...scenario,
    mode,
  }) as typeof scenario;
}

export function setScenarioNetwork(
  scenario: ScenarioV1,
  network: WalletAbiTransportNetwork,
): typeof scenario {
  const nextAssetId = defaultAssetIdForNetwork(network);

  switch (scenario.kind) {
    case "transfer":
      return normalizeScenario({
        ...scenario,
        network,
        recipient: {
          ...scenario.recipient,
          assetId:
            scenario.recipient.assetId.length > 0
              ? scenario.recipient.assetId
              : nextAssetId,
        },
      }) as typeof scenario;
    case "split":
      return normalizeScenario({
        ...scenario,
        network,
        recipients: scenario.recipients.map((recipient) => ({
          ...recipient,
          assetId:
            recipient.assetId.length > 0 ? recipient.assetId : nextAssetId,
        })),
      }) as typeof scenario;
    default:
      return normalizeScenario({
        ...scenario,
        network,
      }) as typeof scenario;
  }
}

export function replaceTransferRecipient(
  scenario: TransferScenario,
  patch: Partial<ScenarioRecipient>,
): TransferScenario {
  return normalizeScenario({
    ...scenario,
    recipient: {
      ...scenario.recipient,
      ...patch,
    },
  }) as TransferScenario;
}

export function replaceSplitRecipient(
  scenario: SplitScenario,
  index: number,
  patch: Partial<ScenarioRecipient>,
): SplitScenario {
  return normalizeScenario({
    ...scenario,
    recipients: scenario.recipients.map((recipient, recipientIndex) =>
      recipientIndex === index
        ? {
            ...recipient,
            ...patch,
          }
        : recipient,
    ),
  }) as SplitScenario;
}

export function addSplitRecipient(
  scenario: SplitScenario,
  recipient = createRecipient(scenario.recipients.length),
): SplitScenario {
  return normalizeScenario({
    ...scenario,
    recipients: [...scenario.recipients, recipient],
  }) as SplitScenario;
}

export function removeSplitRecipient(
  scenario: SplitScenario,
  index: number,
): SplitScenario {
  const recipients = scenario.recipients.filter(
    (_recipient, recipientIndex) => recipientIndex !== index,
  );

  return normalizeScenario({
    ...scenario,
    recipients: recipients.length >= 2 ? recipients : scenario.recipients,
  }) as SplitScenario;
}

export function cloneScenario<TScenario extends ScenarioV1>(
  scenario: TScenario,
): TScenario {
  return normalizeScenario(JSON.parse(JSON.stringify(scenario))) as TScenario;
}
