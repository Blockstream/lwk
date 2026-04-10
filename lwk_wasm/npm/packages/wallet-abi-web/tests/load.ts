import { loadLwkWalletAbiWeb } from "helpers_wallet_abi_web";
import { loadLwkWalletAbiWeb as loadFromHelpers } from "helpers_wallet_abi_web/helpers";

const load: (
  moduleOrPath?: unknown,
) => Promise<typeof import("wallet_abi_sdk_core_web")> = loadLwkWalletAbiWeb;
const loadHelper: (
  moduleOrPath?: unknown,
) => Promise<typeof import("wallet_abi_sdk_core_web")> = loadFromHelpers;

void load;
void loadHelper;
