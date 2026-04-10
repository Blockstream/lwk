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
