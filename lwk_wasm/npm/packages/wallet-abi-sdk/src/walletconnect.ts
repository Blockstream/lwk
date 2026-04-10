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

export interface WalletAbiWalletConnectRequest {
  method: WalletAbiMethod;
  params?: unknown;
}

export interface WalletAbiSessionControllerCallbacks {
  onConnected?(): void;
  onUpdated?(): void;
  onDisconnected?(): void;
}

export interface WalletAbiSessionController {
  readonly chainId: WalletAbiWalletConnectChain;
  session(): SessionTypes.Struct | null;
  connect(): Promise<SessionTypes.Struct>;
  disconnect(): Promise<void>;
  request(request: WalletAbiWalletConnectRequest): Promise<unknown>;
  subscribe(callbacks: WalletAbiSessionControllerCallbacks): () => void;
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

export interface CreateWalletAbiSessionControllerOptions {
  projectId: string;
  network: WalletAbiTransportNetwork;
  appUrl: string;
  metadata?: Partial<WalletAbiMetadata>;
  storagePrefix?: string;
}

interface WalletAbiDisconnectReason {
  code: number;
  message: string;
}

interface WalletAbiAppKitControls {
  open(options?: { uri?: string }): Promise<void>;
  close(): Promise<void>;
}

interface WalletAbiUniversalConnectorInstance {
  appKit: WalletAbiAppKitControls;
}

interface WalletAbiSessionStore {
  getAll(): SessionTypes.Struct[];
  get(topic: string): SessionTypes.Struct;
}

interface WalletAbiSessionSignClient extends WalletAbiSessionApprovalSignClient {
  session: WalletAbiSessionStore;
  connect(input: {
    requiredNamespaces: Record<string, WalletAbiWalletConnectNamespace>;
  }): Promise<{
    uri?: string;
    approval(): Promise<SessionTypes.Struct>;
  }>;
  disconnect(input: { topic: string; reason: WalletAbiDisconnectReason }): Promise<void>;
  request(input: {
    topic: string;
    chainId: WalletAbiWalletConnectChain;
    request: {
      method: WalletAbiMethod;
      params?: object | Record<string, unknown> | unknown[];
    };
  }): Promise<unknown>;
  on(
    event: "session_connect",
    listener: (event: SignClientTypes.EventArguments["session_connect"]) => void
  ): void;
  on(
    event: "session_update",
    listener: (event: SignClientTypes.EventArguments["session_update"]) => void
  ): void;
  on(event: "session_delete", listener: (event: { topic: string }) => void): void;
  on(event: "session_expire", listener: (event: { topic: string }) => void): void;
  off(
    event: "session_connect",
    listener: (event: SignClientTypes.EventArguments["session_connect"]) => void
  ): void;
  off(
    event: "session_update",
    listener: (event: SignClientTypes.EventArguments["session_update"]) => void
  ): void;
  off(event: "session_delete", listener: (event: { topic: string }) => void): void;
  off(event: "session_expire", listener: (event: { topic: string }) => void): void;
}

const DEFAULT_WALLET_ABI_STORAGE_PREFIX = "lwk-wallet-abi-sdk";

function disconnectSession(
  signClient: WalletAbiSessionSignClient,
  topic: string,
  reason: WalletAbiDisconnectReason
): Promise<void> {
  return signClient
    .disconnect({
      topic,
      reason,
    })
    .catch(() => undefined);
}

function createRequiredNamespaces(
  chainId: WalletAbiWalletConnectChain
): Record<string, WalletAbiWalletConnectNamespace> {
  const requiredNamespaces = createWalletAbiRequiredNamespaces(chainId);

  return Object.fromEntries(
    Object.entries(requiredNamespaces).map(([namespace, value]) => [
      namespace,
      {
        methods: [...value.methods],
        chains: [...value.chains],
        events: [...value.events],
      },
    ])
  );
}

function normalizeRequestParams(
  params: unknown
): object | Record<string, unknown> | unknown[] {
  if (Array.isArray(params)) {
    return params;
  }

  if (typeof params === "object" && params !== null) {
    return params as Record<string, unknown>;
  }

  throw new Error("WalletConnect request params must be an object or array");
}

function appKitControls(
  connector: WalletAbiUniversalConnectorInstance
): WalletAbiAppKitControls {
  return connector.appKit;
}

function describeWalletConnectError(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  if (typeof error === "string" && error.trim().length > 0) {
    return error;
  }

  if (typeof error === "object" && error !== null) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim().length > 0) {
      return message;
    }

    try {
      const serialized = JSON.stringify(error);
      if (serialized.length > 0) {
        return serialized;
      }
    } catch {
      // Ignore serialization failures and fall back to a generic string.
    }
  }

  return "Unknown error";
}

class WalletAbiUniversalSessionController
  implements WalletAbiSessionController
{
  readonly chainId: WalletAbiWalletConnectChain;
  readonly #connector: WalletAbiUniversalConnectorInstance;
  readonly #signClient: WalletAbiSessionSignClient;
  readonly #requiredNamespaces: Record<string, WalletAbiWalletConnectNamespace>;
  readonly #disconnectReason: WalletAbiDisconnectReason;

  #session: SessionTypes.Struct | null;

  constructor(
    connector: WalletAbiUniversalConnectorInstance,
    signClient: WalletAbiSessionSignClient,
    session: SessionTypes.Struct | null,
    chainId: WalletAbiWalletConnectChain,
    requiredNamespaces: Record<string, WalletAbiWalletConnectNamespace>,
    disconnectReason: WalletAbiDisconnectReason
  ) {
    this.#connector = connector;
    this.#signClient = signClient;
    this.#session = session;
    this.chainId = chainId;
    this.#requiredNamespaces = requiredNamespaces;
    this.#disconnectReason = disconnectReason;
  }

  session(): SessionTypes.Struct | null {
    if (this.#session !== null) {
      try {
        const nextSession = this.#signClient.session.get(this.#session.topic);
        this.#session = nextSession;
        return nextSession;
      } catch {
        this.#session = null;
      }
    }

    const { activeSession } = selectWalletAbiSessions(
      this.#signClient.session.getAll(),
      this.chainId
    );
    this.#session = activeSession;
    return activeSession;
  }

  async connect(): Promise<SessionTypes.Struct> {
    const existingSession = this.session();
    if (existingSession !== null) {
      return existingSession;
    }

    const appKit = appKitControls(this.#connector);
    let session: SessionTypes.Struct | undefined;
    let modalOpened = false;

    try {
      const { uri, approval } = await this.#signClient.connect({
        requiredNamespaces: this.#requiredNamespaces,
      });

      if (uri !== undefined) {
        await appKit.open({ uri });
        modalOpened = true;
      }

      session = await awaitWalletAbiApprovedSession({
        approval,
        signClient: this.#signClient,
        chainId: this.chainId,
      });
      this.#session = session;
    } catch (error) {
      throw new Error(`Error connecting to wallet: ${describeWalletConnectError(error)}`);
    } finally {
      if (modalOpened) {
        await appKit.close().catch(() => undefined);
      }
    }

    if (session === undefined) {
      throw new Error("Error connecting to wallet: No session found");
    }

    await this.#disconnectExtraSessions(session.topic);
    return session;
  }

  async disconnect(): Promise<void> {
    const session = this.session();
    if (session === null) {
      return;
    }

    this.#session = null;
    await disconnectSession(this.#signClient, session.topic, this.#disconnectReason);
  }

  request(request: WalletAbiWalletConnectRequest): Promise<unknown> {
    const session = this.session();
    if (session === null) {
      throw new Error("WalletConnect session is not connected");
    }

    return this.#signClient.request({
      topic: session.topic,
      chainId: this.chainId,
      request: {
        method: request.method,
        params:
          request.params === undefined ? undefined : normalizeRequestParams(request.params),
      },
    });
  }

  subscribe(callbacks: WalletAbiSessionControllerCallbacks): () => void {
    const onConnected = ({
      session,
    }: SignClientTypes.EventArguments["session_connect"]) => {
      if (!isWalletAbiSession(session, this.chainId)) {
        return;
      }

      this.#session = session;
      callbacks.onConnected?.();
    };
    const onUpdated = ({
      topic,
    }: SignClientTypes.EventArguments["session_update"]) => {
      if (this.#session?.topic !== topic) {
        return;
      }

      try {
        this.#session = this.#signClient.session.get(topic);
      } catch {
        this.#session = null;
        callbacks.onDisconnected?.();
        return;
      }

      callbacks.onUpdated?.();
    };
    const onDisconnected = ({ topic }: { topic: string }) => {
      if (this.#session?.topic !== topic) {
        return;
      }

      this.#session = null;
      callbacks.onDisconnected?.();
    };

    this.#signClient.on("session_connect", onConnected);
    this.#signClient.on("session_update", onUpdated);
    this.#signClient.on("session_delete", onDisconnected);
    this.#signClient.on("session_expire", onDisconnected);

    return () => {
      this.#signClient.off("session_connect", onConnected);
      this.#signClient.off("session_update", onUpdated);
      this.#signClient.off("session_delete", onDisconnected);
      this.#signClient.off("session_expire", onDisconnected);
    };
  }

  async #disconnectExtraSessions(activeTopic: string): Promise<void> {
    const extraSessions = this.#signClient.session
      .getAll()
      .filter(
        (session) => session.topic !== activeTopic && isWalletAbiSession(session, this.chainId)
      );

    await Promise.all(
      extraSessions.map((session) =>
        disconnectSession(this.#signClient, session.topic, this.#disconnectReason)
      )
    );
  }
}

export async function createWalletAbiSessionController({
  projectId,
  network,
  appUrl,
  metadata,
  storagePrefix,
}: CreateWalletAbiSessionControllerOptions): Promise<WalletAbiSessionController> {
  const [{ UniversalConnector }, signClientModule, { getSdkError }] = await Promise.all([
    import("@reown/appkit-universal-connector"),
    import("@walletconnect/sign-client"),
    import("@walletconnect/utils"),
  ]);
  const signClientFactory = signClientModule.default;

  const resolvedMetadata = createWalletAbiMetadata(appUrl, metadata);
  const chainId = walletAbiNetworkToWalletConnectChain(network);
  const caipNetwork = createWalletAbiCaipNetwork(network);
  const requiredNamespaces = createRequiredNamespaces(chainId);
  const disconnectReason = getSdkError("USER_DISCONNECTED") as WalletAbiDisconnectReason;
  const signClient = (await signClientFactory.init({
    projectId,
    metadata: resolvedMetadata,
    customStoragePrefix: storagePrefix ?? DEFAULT_WALLET_ABI_STORAGE_PREFIX,
  })) as WalletAbiSessionSignClient;

  const { activeSession, staleSessions } = selectWalletAbiSessions(
    signClient.session.getAll(),
    chainId
  );
  await Promise.all(
    staleSessions.map((session) =>
      disconnectSession(signClient, session.topic, disconnectReason)
    )
  );

  const connector = (await UniversalConnector.init({
    projectId,
    metadata: resolvedMetadata,
    networks: [
      {
        namespace: WALLET_ABI_WALLETCONNECT_NAMESPACE,
        chains: [caipNetwork],
        methods: [...WALLET_ABI_WALLETCONNECT_METHODS],
        events: [...WALLET_ABI_WALLETCONNECT_EVENTS],
      },
    ],
    modalConfig: {
      themeMode: "light",
    },
    providerConfig: {
      client: signClient as unknown as never,
      ...(activeSession === null ? {} : { session: activeSession }),
    },
  })) as unknown as WalletAbiUniversalConnectorInstance;

  return new WalletAbiUniversalSessionController(
    connector,
    signClient,
    activeSession,
    chainId,
    requiredNamespaces,
    disconnectReason
  );
}
