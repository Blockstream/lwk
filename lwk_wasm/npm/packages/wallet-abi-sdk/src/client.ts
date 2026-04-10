import type {
  WalletAbiJsonRpcRequest,
  WalletAbiJsonRpcResponse,
} from "./protocol.js";
import {
  createGetSignerReceiveAddressRequest,
  parseGetSignerReceiveAddressResponse,
} from "./protocol.js";

type MaybePromise<T> = Promise<T> | T;

export interface WalletAbiRequester {
  connect?(): MaybePromise<void>;
  disconnect?(): MaybePromise<void>;
  request(request: WalletAbiJsonRpcRequest): MaybePromise<WalletAbiJsonRpcResponse>;
}

export interface WalletAbiClientOptions {
  requester: WalletAbiRequester;
  requestTimeoutMs?: number;
}

export class WalletAbiClientError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WalletAbiClientError";
  }
}

export class WalletAbiClient {
  readonly #requester: WalletAbiRequester;
  readonly #requestTimeoutMs: number;

  #connectPromise: Promise<void> | null = null;
  #connected = false;
  #rpcRequestId = 0;

  constructor(options: WalletAbiClientOptions) {
    this.#requester = options.requester;
    this.#requestTimeoutMs = options.requestTimeoutMs ?? 120_000;
  }

  async connect(): Promise<void> {
    if (this.#connected) {
      return;
    }

    if (this.#connectPromise !== null) {
      await this.#connectPromise;
      return;
    }

    this.#connectPromise = (async () => {
      await this.#requester.connect?.();
      this.#connected = true;
    })();

    try {
      await this.#connectPromise;
    } catch (error) {
      this.#connected = false;
      throw error;
    } finally {
      this.#connectPromise = null;
    }
  }

  async disconnect(): Promise<void> {
    if (!this.#connected) {
      return;
    }

    this.#connected = false;
    await this.#requester.disconnect?.();
  }

  requestTimeoutMs(): number {
    return this.#requestTimeoutMs;
  }

  async getSignerReceiveAddress(): Promise<string> {
    await this.connect();

    try {
      const response = await this.#withTimeout(
        Promise.resolve(
          this.#requester.request(
            createGetSignerReceiveAddressRequest(this.#nextRpcRequestId())
          )
        ),
        `wallet request get_signer_receive_address timed out after ${String(this.#requestTimeoutMs)}ms`
      );

      return parseGetSignerReceiveAddressResponse(response);
    } catch (error) {
      if (error instanceof WalletAbiClientError) {
        throw error;
      }

      throw new WalletAbiClientError(normalizeErrorMessage(error));
    }
  }

  #nextRpcRequestId(): number {
    this.#rpcRequestId += 1;
    return this.#rpcRequestId;
  }

  async #withTimeout<T>(promise: Promise<T>, message: string): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;

    const timeoutPromise = new Promise<never>((_, reject) => {
      timer = setTimeout(() => {
        reject(new WalletAbiClientError(message));
      }, this.#requestTimeoutMs);
    });

    try {
      return await Promise.race([promise, timeoutPromise]);
    } finally {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    }
  }
}

function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return String(error);
}
