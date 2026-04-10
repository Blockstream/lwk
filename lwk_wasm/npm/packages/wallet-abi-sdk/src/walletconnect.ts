import type { CustomCaipNetwork } from "@reown/appkit-common";
import type { SessionTypes, SignClientTypes } from "@walletconnect/types";

import {
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  WALLET_ABI_JSON_RPC_VERSION,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
  type WalletAbiMethod,
  type WalletAbiJsonRpcRequest,
  type WalletAbiJsonRpcResponse,
  type WalletAbiTransportNetwork,
} from "./protocol.js";
import type { WalletAbiRequester } from "./client.js";

export const WALLET_ABI_WALLETCONNECT_NAMESPACE = "walabi" as const;

export const WALLET_ABI_WALLETCONNECT_CHAINS = [
  "walabi:liquid",
  "walabi:testnet-liquid",
  "walabi:localtest-liquid",
] as const;
export const WALLET_ABI_WALLETCONNECT_METHODS = [
  GET_SIGNER_RECEIVE_ADDRESS_METHOD,
  GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD,
  WALLET_ABI_PROCESS_REQUEST_METHOD,
] as const;
export const WALLET_ABI_WALLETCONNECT_EVENTS = [] as const;

export type WalletAbiWalletConnectChain =
  (typeof WALLET_ABI_WALLETCONNECT_CHAINS)[number];

export interface WalletAbiWalletConnectNamespace {
  methods: readonly WalletAbiMethod[];
  chains: readonly WalletAbiWalletConnectChain[];
  events: readonly string[];
  accounts?: readonly string[];
}

export interface WalletAbiMetadata {
  name: string;
  description: string;
  url: string;
  icons: string[];
  redirect: {
    universal: string;
  };
}

export interface WalletAbiWalletConnectSessionRequest {
  chainId: WalletAbiWalletConnectChain;
  request: {
    method: WalletAbiMethod;
    params?: unknown;
  };
  topic?: string;
}

export interface WalletAbiWalletConnectClient {
  connect?(): Promise<void> | void;
  disconnect?(): Promise<void> | void;
  request(input: WalletAbiWalletConnectSessionRequest): Promise<unknown> | unknown;
}

export interface CreateWalletConnectRequesterOptions {
  chainId: WalletAbiWalletConnectChain;
  client: WalletAbiWalletConnectClient;
  topic?: string;
  getTopic?(): string | null | undefined;
}

export interface SelectedWalletAbiSessions {
  activeSession: SessionTypes.Struct | null;
  staleSessions: SessionTypes.Struct[];
}

export interface WalletAbiSessionApprovalSignClient {
  session: {
    getAll(): SessionTypes.Struct[];
  };
  on(
    event: "session_connect",
    listener: (event: SignClientTypes.EventArguments["session_connect"]) => void
  ): void;
  off(
    event: "session_connect",
    listener: (event: SignClientTypes.EventArguments["session_connect"]) => void
  ): void;
}

export interface AwaitWalletAbiApprovedSessionOptions {
  approval(): Promise<SessionTypes.Struct>;
  signClient: WalletAbiSessionApprovalSignClient;
  chainId: WalletAbiWalletConnectChain;
  connectTimeoutMs?: number;
  approvalRejectionGraceMs?: number;
  sessionPollMs?: number;
}

const WALLET_ABI_NATIVE_CURRENCY = {
  name: "Liquid Bitcoin",
  symbol: "L-BTC",
  decimals: 8,
} as const;

function walletAbiNetworkName(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "Liquid";
    case "testnet-liquid":
      return "Liquid Testnet";
    case "localtest-liquid":
      return "Liquid Localtest";
  }
}

function walletAbiNetworkRpcUrl(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "https://blockstream.info/liquid/api";
    case "testnet-liquid":
      return "https://blockstream.info/liquidtestnet/api";
    case "localtest-liquid":
      return "http://127.0.0.1:3001";
  }
}

function walletAbiNetworkExplorerUrl(network: WalletAbiTransportNetwork): string {
  switch (network) {
    case "liquid":
      return "https://blockstream.info/liquid";
    case "testnet-liquid":
      return "https://blockstream.info/liquidtestnet";
    case "localtest-liquid":
      return "http://127.0.0.1:3001";
  }
}

export function isWalletAbiWalletConnectChain(
  value: string
): value is WalletAbiWalletConnectChain {
  return WALLET_ABI_WALLETCONNECT_CHAINS.includes(
    value as WalletAbiWalletConnectChain
  );
}

export function walletAbiNetworkToWalletConnectChain(
  network: WalletAbiTransportNetwork
): WalletAbiWalletConnectChain {
  switch (network) {
    case "liquid":
      return "walabi:liquid";
    case "testnet-liquid":
      return "walabi:testnet-liquid";
    case "localtest-liquid":
      return "walabi:localtest-liquid";
  }
}

export function walletConnectChainToWalletAbiNetwork(
  chainId: WalletAbiWalletConnectChain
): WalletAbiTransportNetwork {
  switch (chainId) {
    case "walabi:liquid":
      return "liquid";
    case "walabi:testnet-liquid":
      return "testnet-liquid";
    case "walabi:localtest-liquid":
      return "localtest-liquid";
  }
}

export function createWalletAbiCaipNetwork(
  network: WalletAbiTransportNetwork
): CustomCaipNetwork {
  const caipNetworkId = walletAbiNetworkToWalletConnectChain(network);
  const [, chainReference] = caipNetworkId.split(":");
  const name = walletAbiNetworkName(network);
  const rpcUrl = walletAbiNetworkRpcUrl(network);

  return {
    id: chainReference,
    name,
    caipNetworkId,
    chainNamespace: WALLET_ABI_WALLETCONNECT_NAMESPACE,
    nativeCurrency: WALLET_ABI_NATIVE_CURRENCY,
    rpcUrls: {
      default: {
        http: [rpcUrl],
      },
      public: {
        http: [rpcUrl],
      },
    },
    blockExplorers: {
      default: {
        name: `${name} Explorer`,
        url: walletAbiNetworkExplorerUrl(network),
      },
    },
    testnet: network !== "liquid",
  } as unknown as CustomCaipNetwork;
}

export function createWalletAbiRequiredNamespaces(
  input:
    | WalletAbiTransportNetwork
    | WalletAbiWalletConnectChain
    | readonly WalletAbiWalletConnectChain[]
): Record<
  typeof WALLET_ABI_WALLETCONNECT_NAMESPACE,
  WalletAbiWalletConnectNamespace
> {
  let chains: readonly WalletAbiWalletConnectChain[];

  if (Array.isArray(input)) {
    chains = input;
  } else {
    const singleInput = input as
      | WalletAbiTransportNetwork
      | WalletAbiWalletConnectChain;

    if (isWalletAbiWalletConnectChain(singleInput)) {
      chains = [singleInput];
    } else {
      chains = [walletAbiNetworkToWalletConnectChain(singleInput)];
    }
  }

  return {
    [WALLET_ABI_WALLETCONNECT_NAMESPACE]: {
      methods: WALLET_ABI_WALLETCONNECT_METHODS,
      chains,
      events: WALLET_ABI_WALLETCONNECT_EVENTS,
    },
  };
}

export function createWalletAbiMetadata(
  appUrl: string,
  overrides: Partial<WalletAbiMetadata> = {}
): WalletAbiMetadata {
  const normalizedUrl = new URL(appUrl);

  return {
    name: overrides.name ?? "LWK Wallet ABI SDK",
    description:
      overrides.description ??
      "Wallet ABI WalletConnect session for a browser application.",
    url: overrides.url ?? normalizedUrl.toString(),
    icons: overrides.icons ?? [`${normalizedUrl.origin}/favicon.ico`],
    redirect: overrides.redirect ?? {
      universal: normalizedUrl.toString(),
    },
  };
}

function resolveTopic(
  options: CreateWalletConnectRequesterOptions
): string | undefined {
  const dynamicTopic = options.getTopic?.();
  if (dynamicTopic === null) {
    return undefined;
  }

  if (dynamicTopic !== undefined) {
    return dynamicTopic.trim() || undefined;
  }

  return options.topic?.trim() || undefined;
}

function extractRequestParams(request: WalletAbiJsonRpcRequest): unknown {
  if (request.method === WALLET_ABI_PROCESS_REQUEST_METHOD) {
    return request.params;
  }

  if (
    request.params === undefined ||
    Object.keys(request.params).length === 0
  ) {
    return {};
  }

  return request.params;
}

function createWalletAbiJsonRpcEnvelopeFromResult(
  request: WalletAbiJsonRpcRequest,
  result: unknown
): WalletAbiJsonRpcResponse {
  let normalizedResult = result;

  if (typeof normalizedResult === "string") {
    try {
      normalizedResult = JSON.parse(normalizedResult);
    } catch {
      // Keep plain getter strings untouched.
    }
  }

  return {
    id: request.id,
    jsonrpc: WALLET_ABI_JSON_RPC_VERSION,
    result: normalizedResult,
  } as WalletAbiJsonRpcResponse;
}

export function createWalletConnectRequester(
  options: CreateWalletConnectRequesterOptions
): WalletAbiRequester {
  return {
    connect() {
      return options.client.connect?.();
    },
    disconnect() {
      return options.client.disconnect?.();
    },
    async request(request) {
      const topic = resolveTopic(options);
      const params = extractRequestParams(request);
      const result = await options.client.request({
        chainId: options.chainId,
        ...(topic === undefined ? {} : { topic }),
        request: {
          method: request.method,
          ...(params === undefined ? {} : { params }),
        },
      });

      return createWalletAbiJsonRpcEnvelopeFromResult(request, result);
    },
  };
}

function sessionContainsChain(
  session: SessionTypes.Struct,
  chainId: WalletAbiWalletConnectChain
): boolean {
  const namespace = session.namespaces[WALLET_ABI_WALLETCONNECT_NAMESPACE];
  if (namespace === undefined) {
    return false;
  }

  if (namespace.accounts.some((account) => account.startsWith(`${chainId}:`))) {
    return true;
  }

  if (namespace.chains?.includes(chainId) === true) {
    return true;
  }

  return (
    session.requiredNamespaces[WALLET_ABI_WALLETCONNECT_NAMESPACE]?.chains?.includes(
      chainId
    ) === true
  );
}

export function isWalletAbiSession(
  session: SessionTypes.Struct,
  chainId: WalletAbiWalletConnectChain
): boolean {
  const namespace = session.namespaces[WALLET_ABI_WALLETCONNECT_NAMESPACE];
  if (namespace === undefined) {
    return false;
  }

  const supportsMethods = WALLET_ABI_WALLETCONNECT_METHODS.every((method) =>
    namespace.methods.includes(method)
  );

  return supportsMethods && sessionContainsChain(session, chainId);
}

export function selectWalletAbiSessions(
  sessions: SessionTypes.Struct[],
  chainId: WalletAbiWalletConnectChain
): SelectedWalletAbiSessions {
  const matchingSessions = sessions
    .filter((session) => isWalletAbiSession(session, chainId))
    .sort((left, right) => right.expiry - left.expiry);

  const [activeSession, ...staleSessions] = matchingSessions;
  return {
    activeSession: activeSession ?? null,
    staleSessions,
  };
}

function currentWalletAbiSession(
  signClient: WalletAbiSessionApprovalSignClient,
  chainId: WalletAbiWalletConnectChain
): SessionTypes.Struct | null {
  return selectWalletAbiSessions(signClient.session.getAll(), chainId).activeSession;
}

function sleep(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message: string
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timeoutId = setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);

    promise.then(
      (result) => {
        clearTimeout(timeoutId);
        resolve(result);
      },
      (error) => {
        clearTimeout(timeoutId);
        reject(error);
      }
    );
  });
}

export async function awaitWalletAbiApprovedSession({
  approval,
  signClient,
  chainId,
  connectTimeoutMs = 90_000,
  approvalRejectionGraceMs = 1_500,
  sessionPollMs = 250,
}: AwaitWalletAbiApprovedSessionOptions): Promise<SessionTypes.Struct> {
  const existingSession = currentWalletAbiSession(signClient, chainId);
  if (existingSession !== null) {
    return existingSession;
  }

  let active = true;
  let cleanupListener: (() => void) | null = null;

  const stopWaiting = () => {
    if (!active) {
      return;
    }

    active = false;
    cleanupListener?.();
    cleanupListener = null;
  };

  const sessionConnectPromise = new Promise<SessionTypes.Struct>((resolve) => {
    const onConnected = ({
      session,
    }: SignClientTypes.EventArguments["session_connect"]) => {
      if (!isWalletAbiSession(session, chainId)) {
        return;
      }

      stopWaiting();
      resolve(session);
    };

    cleanupListener = () => {
      signClient.off("session_connect", onConnected);
    };
    signClient.on("session_connect", onConnected);
  });

  const approvalPromise = approval()
    .then((session) => {
      stopWaiting();
      return session;
    })
    .catch(async (error) => {
      const deadline = Date.now() + approvalRejectionGraceMs;

      while (active && Date.now() < deadline) {
        const nextSession = currentWalletAbiSession(signClient, chainId);
        if (nextSession !== null) {
          stopWaiting();
          return nextSession;
        }

        await sleep(sessionPollMs);
      }

      stopWaiting();
      throw error;
    });

  try {
    return await withTimeout(
      Promise.race([approvalPromise, sessionConnectPromise]),
      connectTimeoutMs,
      "WalletConnect session approval timed out"
    );
  } finally {
    stopWaiting();
  }
}
