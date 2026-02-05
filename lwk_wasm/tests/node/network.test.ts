import { expect } from "chai";
import * as lwk from "lwk_node";

describe("Network", function () {
  it("should create testnet network", function () {
    const network = lwk.Network.testnet();
    expect(network.toString()).to.equal("LiquidTestnet");
  });
});
