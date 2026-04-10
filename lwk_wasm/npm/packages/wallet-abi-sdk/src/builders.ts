import {
  OutPoint,
  TxSequence,
  WalletAbiFinalizerSpec,
  WalletAbiInputIssuance,
  WalletAbiInputSchema,
  WalletAbiInputUnblinding,
  WalletAbiUtxoSource,
  WalletAbiWalletSourceFilter,
} from "./schema.js";

export function generateRequestId(): string {
  const cryptoApi = Reflect.get(globalThis, "crypto") as
    | {
        randomUUID?: () => string;
      }
    | undefined;

  if (typeof cryptoApi?.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }

  throw new Error(
    "Wallet ABI SDK requires globalThis.crypto.randomUUID() support."
  );
}

export function createWalletInput(input: {
  id: string;
  filter?: WalletAbiWalletSourceFilter;
  unblinding?: WalletAbiInputUnblinding;
  sequence?: TxSequence;
  finalizer?: WalletAbiFinalizerSpec;
  issuance?: WalletAbiInputIssuance;
}): WalletAbiInputSchema {
  let schema = WalletAbiInputSchema.fromSequence(
    input.id,
    WalletAbiUtxoSource.wallet(input.filter ?? WalletAbiWalletSourceFilter.any()),
    input.unblinding ?? WalletAbiInputUnblinding.wallet(),
    input.sequence ?? TxSequence.max(),
    input.finalizer ?? WalletAbiFinalizerSpec.wallet()
  );

  if (input.issuance !== undefined) {
    schema = schema.withIssuance(input.issuance);
  }

  return schema;
}

export function createProvidedInput(input: {
  id: string;
  outpoint: OutPoint;
  unblinding?: WalletAbiInputUnblinding;
  sequence?: TxSequence;
  finalizer?: WalletAbiFinalizerSpec;
  issuance?: WalletAbiInputIssuance;
}): WalletAbiInputSchema {
  let schema = WalletAbiInputSchema.fromSequence(
    input.id,
    WalletAbiUtxoSource.provided(input.outpoint),
    input.unblinding ?? WalletAbiInputUnblinding.explicit(),
    input.sequence ?? TxSequence.max(),
    input.finalizer ?? WalletAbiFinalizerSpec.wallet()
  );

  if (input.issuance !== undefined) {
    schema = schema.withIssuance(input.issuance);
  }

  return schema;
}
