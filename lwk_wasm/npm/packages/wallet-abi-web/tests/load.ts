import { loadLwkWalletAbiWeb } from "lwk_wallet_abi_web";
import { loadLwkWalletAbiWeb as loadFromHelpers } from "lwk_wallet_abi_web/helpers";

const load: (moduleOrPath?: unknown) => Promise<typeof import("lwk_web")> =
  loadLwkWalletAbiWeb;
const loadHelper: (moduleOrPath?: unknown) => Promise<
  typeof import("lwk_web")
> = loadFromHelpers;

void load;
void loadHelper;
