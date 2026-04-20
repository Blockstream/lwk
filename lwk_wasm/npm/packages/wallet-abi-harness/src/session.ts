import {
  WalletAbiClient,
  createGetRawSigningXOnlyPubkeyRequest,
  createGetSignerReceiveAddressRequest,
  createWalletAbiSessionController,
  createWalletConnectRequester,
  type WalletAbiJsonRpcRequest,
  parseGetRawSigningXOnlyPubkeyResponse,
  parseGetSignerReceiveAddressResponse,
  parseProcessRequestResponse,
  type WalletAbiJsonRpcResponse,
  type WalletAbiSessionController,
  type WalletAbiSessionControllerCallbacks,
  type WalletAbiTxCreateRequest,
} from "lwk_wallet_abi_sdk";

import { createProcessRequest } from "lwk_wallet_abi_sdk/protocol";

import {
  createRawSuccessEnvelope,
  type HarnessRawSuccessEnvelope,
} from "./request.js";
import type { HarnessRawEnvelope } from "./url.js";
import { createTranscriptEntry, type TranscriptEntry } from "./transcript.js";

interface SendLoggedRequestOptions<
  TValue,
  TRequest extends WalletAbiJsonRpcRequest,
> {
  request: TRequest;
  parse(response: WalletAbiJsonRpcResponse): TValue;
}

export interface HarnessSessionOptions {
  projectId: string;
  network: "liquid" | "testnet-liquid" | "localtest-liquid";
  appUrl: string;
  storagePrefix?: string;
  requestTimeoutMs?: number;
  onTranscript?(entry: TranscriptEntry): void;
}

export interface HarnessSession {
  readonly chainId: WalletAbiSessionController["chainId"];
  readonly client: WalletAbiClient;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  sessionTopic(): string | null;
  subscribe(callbacks: WalletAbiSessionControllerCallbacks): () => void;
  getSignerReceiveAddress(): Promise<{
    value: string;
    response: WalletAbiJsonRpcResponse;
  }>;
  getRawSigningXOnlyPubkey(): Promise<{
    value: string;
    response: WalletAbiJsonRpcResponse;
  }>;
  processRequest(request: WalletAbiTxCreateRequest): Promise<{
    response: WalletAbiJsonRpcResponse;
    value: ReturnType<typeof parseProcessRequestResponse>;
  }>;
  sendRawEnvelope(envelope: HarnessRawEnvelope): Promise<{
    response: HarnessRawSuccessEnvelope;
  }>;
}

function normalizeTopic(controller: WalletAbiSessionController): string | null {
  return controller.session()?.topic ?? null;
}

function describeError(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return String(error);
}

function normalizeJsonRpcResult(result: unknown): unknown {
  if (typeof result !== "string") {
    return result;
  }

  try {
    return JSON.parse(result);
  } catch {
    return result;
  }
}

function createHarnessSessionFromController(
  controller: WalletAbiSessionController,
  options: Omit<
    HarnessSessionOptions,
    "projectId" | "network" | "appUrl" | "storagePrefix"
  >,
): HarnessSession {
  const requester = createWalletConnectRequester({
    chainId: controller.chainId,
    getTopic: () => controller.session()?.topic,
    client: {
      async connect() {
        await controller.connect();
      },
      disconnect: () => controller.disconnect(),
      request: ({ request }) => controller.request(request),
    },
  });
  const client = new WalletAbiClient({
    requester,
    requestTimeoutMs: options.requestTimeoutMs,
  });
  let rpcId = 0;

  function nextRpcId(): number {
    rpcId += 1;
    return rpcId;
  }

  function emit(entry: Omit<TranscriptEntry, "id">): void {
    options.onTranscript?.(createTranscriptEntry(entry));
  }

  async function ensureConnected(): Promise<void> {
    await client.connect();
  }

  async function sendLoggedRequest<
    TValue,
    TRequest extends WalletAbiJsonRpcRequest,
  >({
    request,
    parse,
  }: SendLoggedRequestOptions<TValue, TRequest>): Promise<{
    value: TValue;
    response: WalletAbiJsonRpcResponse;
  }> {
    await ensureConnected();
    const topic = normalizeTopic(controller);
    const startedAt = Date.now();

    emit({
      direction: "outbound",
      method: request.method,
      rpcId: String(request.id),
      topic,
      timestampMs: startedAt,
      elapsedMs: null,
      payload: request,
      status: "ok",
    });

    try {
      const response = await requester.request(request);
      const finishedAt = Date.now();

      emit({
        direction: "inbound",
        method: request.method,
        rpcId: String(request.id),
        topic: normalizeTopic(controller),
        timestampMs: finishedAt,
        elapsedMs: finishedAt - startedAt,
        payload: response,
        status: "ok",
      });

      return {
        response,
        value: parse(response),
      };
    } catch (error) {
      const finishedAt = Date.now();

      emit({
        direction: "inbound",
        method: request.method,
        rpcId: String(request.id),
        topic: normalizeTopic(controller),
        timestampMs: finishedAt,
        elapsedMs: finishedAt - startedAt,
        payload: {
          error: describeError(error),
        },
        status: "error",
      });

      throw error;
    }
  }

  return {
    chainId: controller.chainId,
    client,
    async connect() {
      await client.connect();
    },
    async disconnect() {
      await client.disconnect();
    },
    sessionTopic() {
      return normalizeTopic(controller);
    },
    subscribe(callbacks) {
      return controller.subscribe(callbacks);
    },
    getSignerReceiveAddress() {
      return sendLoggedRequest({
        request: createGetSignerReceiveAddressRequest(nextRpcId()),
        parse: parseGetSignerReceiveAddressResponse,
      });
    },
    getRawSigningXOnlyPubkey() {
      return sendLoggedRequest({
        request: createGetRawSigningXOnlyPubkeyRequest(nextRpcId()),
        parse: parseGetRawSigningXOnlyPubkeyResponse,
      });
    },
    processRequest(request) {
      return sendLoggedRequest({
        request: createProcessRequest(nextRpcId(), request),
        parse: parseProcessRequestResponse,
      });
    },
    async sendRawEnvelope(envelope) {
      await ensureConnected();
      const topic = normalizeTopic(controller);
      const startedAt = Date.now();

      emit({
        direction: "outbound",
        method: envelope.method,
        rpcId: String(envelope.id),
        topic,
        timestampMs: startedAt,
        elapsedMs: null,
        payload: envelope,
        status: "ok",
      });

      try {
        const result = await controller.request({
          method: envelope.method,
          ...(Object.prototype.hasOwnProperty.call(envelope, "params")
            ? { params: envelope.params }
            : {}),
        });
        const response = createRawSuccessEnvelope(
          envelope,
          normalizeJsonRpcResult(result),
        );
        const finishedAt = Date.now();

        emit({
          direction: "inbound",
          method: envelope.method,
          rpcId: String(envelope.id),
          topic: normalizeTopic(controller),
          timestampMs: finishedAt,
          elapsedMs: finishedAt - startedAt,
          payload: response,
          status: "ok",
        });

        return {
          response,
        };
      } catch (error) {
        const finishedAt = Date.now();

        emit({
          direction: "inbound",
          method: envelope.method,
          rpcId: String(envelope.id),
          topic: normalizeTopic(controller),
          timestampMs: finishedAt,
          elapsedMs: finishedAt - startedAt,
          payload: {
            error: describeError(error),
          },
          status: "error",
        });

        throw error;
      }
    },
  };
}

export async function createHarnessSession(
  options: HarnessSessionOptions,
): Promise<HarnessSession> {
  const controller = await createWalletAbiSessionController({
    projectId: options.projectId,
    network: options.network,
    appUrl: options.appUrl,
    storagePrefix: options.storagePrefix,
    metadata: {
      name: "Wallet ABI Harness",
      description:
        "Builder and WalletConnect test harness for Liquid Wallet Kit Wallet ABI flows.",
    },
  });

  return createHarnessSessionFromController(controller, options);
}

export { createHarnessSessionFromController };
