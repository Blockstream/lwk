import * as lwkWeb from "lwk_web";

type LwkWalletAbiWebModule = typeof import("lwk_web");
type LwkWalletAbiWebInitModule = LwkWalletAbiWebModule & {
  default?: (moduleOrPath?: unknown) => Promise<unknown>;
};

let initPromise: Promise<LwkWalletAbiWebModule> | undefined;

export function loadLwkWalletAbiWeb(
  moduleOrPath?: unknown
): Promise<LwkWalletAbiWebModule> {
  if (initPromise) {
    return initPromise;
  }

  const module = lwkWeb as LwkWalletAbiWebInitModule;
  const promise = (async (): Promise<LwkWalletAbiWebModule> => {
    if (typeof module.default === "function") {
      await module.default(moduleOrPath);
    }

    return lwkWeb;
  })();

  initPromise = promise.catch((error: unknown) => {
    initPromise = undefined;
    throw error;
  });

  return initPromise;
}
