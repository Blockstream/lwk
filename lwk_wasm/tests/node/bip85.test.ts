import { expect } from "chai";
import * as lwk from "lwk_node";

// TODO: use regtest instead of testnet:
// however keep displaying testnet in the generated docs using // ANCHOR: ignore

describe("Bip85", function () {
  it("should derive BIP85 mnemonics", async function () {
    // ANCHOR: bip85
    // Load mnemonic
    const mnemonic = new lwk.Mnemonic(
      "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    );

    // Create signer
    const network = lwk.Network.testnet();
    const signer = new lwk.Signer(mnemonic, network);

    // Derive menmonics
    const derived_0_12 = await signer.derive_bip85_mnemonic(0, 12);
    const derived_0_24 = await signer.derive_bip85_mnemonic(0, 24);
    const derived_1_12 = await signer.derive_bip85_mnemonic(1, 12);
    // ANCHOR_END: bip85

    expect(derived_0_12.toString()).to.equal(
      "prosper short ramp prepare exchange stove life snack client enough purpose fold"
    );
    expect(derived_0_24.toString()).to.equal(
      "stick exact spice sock filter ginger museum horse kit multiply manual wear grief demand derive alert quiz fault december lava picture immune decade jaguar"
    );
    expect(derived_1_12.toString()).to.equal(
      "sing slogan bar group gauge sphere rescue fossil loyal vital model desert"
    );
  });
});
