import { Network, networkFromString } from "lwk_wallet_abi_web";
import { networkFromString as networkFromHelpers } from "lwk_wallet_abi_web/helpers";

const mainnet: Network = networkFromString("liquid");
const testnet: Network = networkFromString("liquid-testnet");
const regtestDefault: Network = networkFromString("liquid-regtest");
const regtestCustom: Network = networkFromHelpers(
  "liquid-regtest",
  "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225",
);

void mainnet;
void testnet;
void regtestDefault;
void regtestCustom;
