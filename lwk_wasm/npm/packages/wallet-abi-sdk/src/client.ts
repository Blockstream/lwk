import type {
  WalletAbiJsonRpcRequest,
  WalletAbiJsonRpcResponse,
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
}
