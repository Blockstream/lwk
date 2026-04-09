import java.util.concurrent.atomic.AtomicInteger
import uniffi.lwk.*

val network = Network.testnet()
val mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
val signer = Signer(mnemonic, network)
val walletDescriptor = signer.wpkhSlip77Descriptor()
val wallet = Wollet(network, walletDescriptor, null)
val walletAddress = wallet.address(0u).address()
val externalAddress = Address(
    "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
)
val expectedXonly = simplicityDeriveXonlyPubkey(signer, "m/86h/1h/0h/0/0")
val derivationPair = walletAbiBip32DerivationPairFromSigner(
    signer,
    listOf(2147483732u, 2147483649u, 2147483648u, 0u, 0u),
)
val policyAsset = network.policyAsset()
val outpoint = OutPoint.fromParts(
    Txid("0000000000000000000000000000000000000000000000000000000000000001"),
    0u,
)
val txOut = TxOut.fromExplicit(walletAddress.scriptPubkey(), policyAsset, 20_000uL)
val txOutSecrets = TxOutSecrets.fromExplicit(policyAsset, 20_000uL)
val utxo = ExternalUtxo.fromUncheckedData(outpoint, txOut, txOutSecrets, 107u)
val requestId = "0d6d53cd-a040-4f0c-8d28-c67b6608fb14"

val openSessionCalls = AtomicInteger(0)
val outputTemplateCalls = AtomicInteger(0)
val prevoutCalls = AtomicInteger(0)
val broadcastCalls = AtomicInteger(0)
val receiveAddressCalls = AtomicInteger(0)
val xonlyGetterCalls = AtomicInteger(0)
val signPstCalls = AtomicInteger(0)
val signSchnorrCalls = AtomicInteger(0)

val signerLink = SignerMetaLink(
    object : WalletAbiSignerCallbacks {
        override fun getRawSigningXOnlyPubkey(): XOnlyPublicKey {
            xonlyGetterCalls.incrementAndGet()
            return expectedXonly
        }

        override fun signPst(pst: Pset): Pset {
            signPstCalls.incrementAndGet()
            return signer.sign(pst)
        }

        override fun signSchnorr(message: ByteArray): ByteArray {
            signSchnorrCalls.incrementAndGet()
            return ByteArray(64)
        }
    }
)

val sessionFactory = WalletSessionFactoryLink(
    object : WalletAbiSessionFactoryCallbacks {
        override fun openWalletRequestSession(): WalletAbiRequestSession {
            openSessionCalls.incrementAndGet()
            return WalletAbiRequestSession(
                "session-1",
                network,
                listOf(utxo),
            )
        }
    }
)

val outputAllocator = WalletOutputAllocatorLink(
    object : WalletAbiOutputAllocatorCallbacks {
        override fun getWalletOutputTemplate(
            session: WalletAbiRequestSession,
            request: WalletAbiWalletOutputRequest,
        ): WalletAbiWalletOutputTemplate {
            outputTemplateCalls.incrementAndGet()
            check(session.spendableUtxos.isNotEmpty())
            check(request.ordinal >= 0u)
            return walletAbiOutputTemplateFromAddress(walletAddress)
        }
    }
)

val prevoutResolver = WalletPrevoutResolverLink(
    object : WalletAbiPrevoutResolverCallbacks {
        override fun getBip32DerivationPair(outpoint: OutPoint): WalletAbiBip32DerivationPair? {
            prevoutCalls.incrementAndGet()
            check(outpoint.toString().isNotEmpty())
            return derivationPair
        }

        override fun unblind(txOut: TxOut): TxOutSecrets {
            prevoutCalls.incrementAndGet()
            check(txOut.scriptPubkey().toString().isNotEmpty())
            return txOutSecrets
        }

        override fun getTxOut(outpoint: OutPoint): TxOut {
            prevoutCalls.incrementAndGet()
            check(outpoint.toString().isNotEmpty())
            return txOut
        }
    }
)

val broadcaster = WalletBroadcasterLink(
    object : WalletAbiBroadcasterCallbacks {
        override fun broadcastTransaction(tx: Transaction): Txid {
            broadcastCalls.incrementAndGet()
            return tx.txid()
        }
    }
)

val receiveAddressProvider = WalletReceiveAddressProviderLink(
    object : WalletAbiReceiveAddressProviderCallbacks {
        override fun getSignerReceiveAddress(): Address {
            receiveAddressCalls.incrementAndGet()
            return walletAddress
        }
    }
)

val runtimeDeps = WalletRuntimeDepsLink(
    sessionFactory,
    outputAllocator,
    prevoutResolver,
    broadcaster,
    receiveAddressProvider,
)
val provider = WalletAbiProvider(signerLink, runtimeDeps)

val input = WalletAbiInputSchema.fromSequence(
    "wallet-input",
    WalletAbiUtxoSource.wallet(
        WalletAbiWalletSourceFilter.withFilters(
            WalletAbiAssetFilter.exact(policyAsset),
            WalletAbiAmountFilter.exact(20_000uL),
            WalletAbiLockFilter.none(),
        )
    ),
    WalletAbiInputUnblinding.wallet(),
    TxSequence.max(),
    WalletAbiFinalizerSpec.wallet(),
)
val output = WalletAbiOutputSchema(
    "external",
    5_000uL,
    WalletAbiLockVariant.script(externalAddress.scriptPubkey()),
    WalletAbiAssetVariant.assetId(policyAsset),
    WalletAbiBlinderVariant.explicit(),
)
val params = WalletAbiRuntimeParams(
    listOf(input),
    listOf(output),
    0.0f,
    null,
)
val createRequest = WalletAbiTxCreateRequest.fromParts(requestId, network, params, true)
val evaluateRequest = WalletAbiTxEvaluateRequest.fromParts(requestId, network, params)

check(provider.getRawSigningXOnlyPubkey().toString() == expectedXonly.toString())
check(provider.getSignerReceiveAddress().toString() == walletAddress.toString())

val capabilities = provider.getCapabilities()
check(capabilities.network() == network)
check(
    capabilities.methods() == listOf(
        "get_raw_signing_x_only_pubkey",
        "get_signer_receive_address",
        "wallet_abi_evaluate_request",
        "wallet_abi_get_capabilities",
        "wallet_abi_process_request",
    )
)

val createResponse = provider.processRequest(createRequest)
check(createResponse.transaction() != null)
check(createResponse.preview() != null)
check(createResponse.errorInfo() == null)

val evaluateResponse = provider.evaluateRequest(evaluateRequest)
check(evaluateResponse.preview() != null)
check(evaluateResponse.errorInfo() == null)

check(provider.dispatchJson("get_raw_signing_x_only_pubkey", "null") == "\"${expectedXonly}\"")
check(provider.dispatchJson("get_signer_receive_address", "null") == "\"${walletAddress}\"")
val dispatchCapabilities = WalletAbiCapabilities.fromJson(
    provider.dispatchJson("wallet_abi_get_capabilities", "null")
)
check(dispatchCapabilities.network() == capabilities.network())
check(dispatchCapabilities.methods() == capabilities.methods())

val dispatchCreateResponse = WalletAbiTxCreateResponse.fromJson(
    provider.dispatchJson("wallet_abi_process_request", createRequest.toJson())
)
check(dispatchCreateResponse.status() == createResponse.status())
check(dispatchCreateResponse.transaction() != null)
check(dispatchCreateResponse.preview() != null)
check(dispatchCreateResponse.errorInfo() == null)

val dispatchEvaluateResponse = WalletAbiTxEvaluateResponse.fromJson(
    provider.dispatchJson("wallet_abi_evaluate_request", evaluateRequest.toJson())
)
check(dispatchEvaluateResponse.status() == evaluateResponse.status())
check(dispatchEvaluateResponse.preview() != null)
check(dispatchEvaluateResponse.errorInfo() == null)

check(openSessionCalls.get() >= 1)
check(outputTemplateCalls.get() >= 1)
check(prevoutCalls.get() >= 1)
check(broadcastCalls.get() >= 1)
check(receiveAddressCalls.get() >= 1)
check(xonlyGetterCalls.get() >= 1)
check(signPstCalls.get() >= 1)
check(signSchnorrCalls.get() == 0)
