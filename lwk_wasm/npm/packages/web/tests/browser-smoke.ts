import * as lwk from "@blockstream/lwk-web";

const network = lwk.Network.testnet();
const policyAsset = network.policyAsset().toString();

console.log("browser smoke", network.toString(), policyAsset);

export { network, policyAsset };
