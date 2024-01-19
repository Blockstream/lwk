package com.blockstream.lwk_bindings

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import lwk_bindings.Mnemonic
import lwk_bindings.NetworkBuilder
import lwk_bindings.Signer
import lwk_bindings.Wollet

import org.junit.Test
import org.junit.runner.RunWith

import org.junit.Assert.*

/**
 * Instrumented test, which will execute on an Android device.
 *
 * See [testing documentation](http://d.android.com/tools/testing).
 */
@RunWith(AndroidJUnit4::class)
class InstrumentedTest {

    @Test
    fun test() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext

        val mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
        val network = NetworkBuilder().testnet()
        val client = network.defaultElectrumClient()

        val signer = Signer(mnemonic, network)
        val desc = signer.wpkhSlip77Descriptor()

        println(desc)
        // ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d

        val w = Wollet(network, desc, context.filesDir.absolutePath)
        val update = client.fullScan(w)!!
        w.applyUpdate(update)

        w.balance()
        // # {'144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49': 100000}

        println("-- Balance ---")
        w.balance().forEach { asset, balance ->
            println("assetId: $asset balance: $balance")
        }

        println("-- Transactions ---")
        w.transactions().forEach {
            println(it.txid().toString())
        }
    }
}