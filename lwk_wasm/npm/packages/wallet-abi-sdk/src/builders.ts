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
