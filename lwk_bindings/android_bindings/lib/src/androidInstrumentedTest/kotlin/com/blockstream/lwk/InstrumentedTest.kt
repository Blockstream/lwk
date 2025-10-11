package com.blockstream.lwk

import androidx.test.ext.junit.runners.AndroidJUnit4
import lwk.Mnemonic
import lwk.Network
import lwk.Signer
import lwk.Wollet
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented test, which will execute on an Android device.
 *
 * See [testing documentation](http://d.android.com/tools/testing).
 */
@RunWith(AndroidJUnit4::class)
class InstrumentedTest {

    @Test
    fun test() {
        // Copied to lwk_bindings/tests/bindings/list_transactions.kts cause there it's enforced in CI

        val network = Network.testnet()

        assert(network.toString() == "LiquidTestnet")

        val client = network.defaultElectrumClient()

        val mnemonic =
            Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")

        val signer = Signer(mnemonic, network)
        val desc = signer.wpkhSlip77Descriptor()

        assert(desc.toString() == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d")

        val w = Wollet(network, desc, null)
        val update = client.fullScan(w)!!

        w.applyUpdate(update)

        assert(!w.transactions().isEmpty())
    }
}