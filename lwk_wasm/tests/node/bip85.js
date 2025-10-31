const lwk = require('lwk_node');

// TODO: use regtest instead of testnet:
// however keep displaying testnet in the generated docs using // ANCHOR: ignore

async function runBip85Test() {
    try {
        // ANCHOR: bip85
        // Load mnemonic
        const mnemonic = new lwk.Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");

        // Create signer
        const network = lwk.Network.testnet();
        const signer = new lwk.Signer(mnemonic, network);
        console.log("Signer created");

        // Derive menmonics
        const derived_0_12 = await signer.derive_bip85_mnemonic(0, 12);
        const derived_0_24 = await signer.derive_bip85_mnemonic(0, 24);
        const derived_1_12 = await signer.derive_bip85_mnemonic(1, 12);
        // ANCHOR_END: bip85

        console.assert(derived_0_12.toString() === "prosper short ramp prepare exchange stove life snack client enough purpose fold");
        console.assert(derived_0_24.toString() === "stick exact spice sock filter ginger museum horse kit multiply manual wear grief demand derive alert quiz fault december lava picture immune decade jaguar");
        console.assert(derived_1_12.toString() === "sing slogan bar group gauge sphere rescue fossil loyal vital model desert");
    } catch (error) {
	console.error("Bip85 test failed:", error);
	throw error;
    }
    console.log("Bip85 test passed");
}

if (require.main === module) {
    runBip85Test();
}

module.exports = { runBip85Test };
