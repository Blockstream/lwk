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
        // MOVED to lwk_bindings/tests/bindings/list_transactions.kts cause there it's enforced in CI
    }
}