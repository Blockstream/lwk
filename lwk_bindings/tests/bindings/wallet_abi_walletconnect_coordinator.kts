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

val openSessionCalls = AtomicInteger(0)
val outputTemplateCalls = AtomicInteger(0)
val broadcastCalls = AtomicInteger(0)

val signerLink = SignerMetaLink(
    object : WalletAbiSignerCallbacks {
        override fun getRawSigningXOnlyPubkey(): XOnlyPublicKey = expectedXonly

        override fun signPst(pst: Pset): Pset = signer.sign(pst)

        override fun signSchnorr(message: ByteArray): ByteArray = ByteArray(64)
    }
)

val sessionFactory = WalletSessionFactoryLink(
    object : WalletAbiSessionFactoryCallbacks {
        override fun openWalletRequestSession(): WalletAbiRequestSession {
            openSessionCalls.incrementAndGet()
            return WalletAbiRequestSession(
                "frozen-session",
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
            check(session.sessionId == "frozen-session")
            check(request.ordinal >= 0u)
            return walletAbiOutputTemplateFromAddress(walletAddress)
        }
    }
)

val prevoutResolver = WalletPrevoutResolverLink(
    object : WalletAbiPrevoutResolverCallbacks {
        override fun getBip32DerivationPair(outpoint: OutPoint): WalletAbiBip32DerivationPair? =
            derivationPair

        override fun unblind(txOut: TxOut): TxOutSecrets = txOutSecrets

        override fun getTxOut(outpoint: OutPoint): TxOut = txOut
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
        override fun getSignerReceiveAddress(): Address = walletAddress
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
val coordinator = WalletAbiWalletConnectCoordinator(provider, "wallet-1")

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
val createRequest = WalletAbiTxCreateRequest.fromParts(
    "0d6d53cd-a040-4f0c-8d28-c67b6608fb14",
    network,
    params,
    true,
)

val proposal = WalletAbiWalletConnectSessionProposal(
    7uL,
    "wc:abc@2?symKey=01",
    "Requester",
    "Wallet ABI harness",
    "https://example.com",
    emptyList(),
    listOf("walabi:testnet-liquid"),
    emptyList(),
    listOf(
        "get_signer_receive_address",
        "get_raw_signing_x_only_pubkey",
        "wallet_abi_process_request",
    ),
    emptyList(),
    emptyList(),
    emptyList(),
)

check(coordinator.handleSessionProposal(proposal).isEmpty())
check(coordinator.uiState().currentOverlay != null)

val sessionApprove = coordinator.approveCurrentOverlay()
check(sessionApprove.size == 1)
check(sessionApprove.single().approveSession != null)
coordinator.handleApproveSessionSucceeded(
    sessionApprove.single().actionId,
    WalletAbiWalletConnectSessionInfo(
        "topic-1",
        "walabi:testnet-liquid",
        listOf(
            "get_signer_receive_address",
            "get_raw_signing_x_only_pubkey",
            "wallet_abi_process_request",
        ),
        listOf("walabi:testnet-liquid:wallet-1"),
        "Requester",
        "Wallet ABI harness",
        "https://example.com",
        emptyList(),
    )
)

val requestRecord = WalletAbiWalletConnectSessionRequest(
    "topic-1",
    42uL,
    "walabi:testnet-liquid",
    "wallet_abi_process_request",
    createRequest.toJson(),
)

check(coordinator.handleSessionRequest(requestRecord).isEmpty())
val overlay = coordinator.uiState().currentOverlay
check(overlay != null)
check(overlay.previewJson != null)
check(WalletAbiRequestPreview.fromJson(overlay.previewJson!!).outputs().isNotEmpty())

val pendingSnapshot = coordinator.snapshotJson()
val restoredPending = WalletAbiWalletConnectCoordinator.fromSnapshotJson(
    provider,
    "wallet-1",
    pendingSnapshot,
)
check(restoredPending.uiState().currentOverlay?.previewJson != null) { "restored pending overlay missing preview" }
val pendingReconcile = restoredPending.reconcilePendingRequests(listOf(requestRecord))
check(pendingReconcile.actions.isEmpty()) { "pending reconcile unexpectedly emitted actions" }
check(pendingReconcile.requestsToReplay.isEmpty()) { "pending reconcile unexpectedly asked for replay" }

val responseActions = coordinator.approveCurrentOverlay()
check(responseActions.size == 1) { "expected one response action, got ${responseActions.size}" }
check(responseActions.single().respondSuccess != null) { "expected respondSuccess action" }
coordinator.handleTransportActionSucceeded(responseActions.single().actionId)

check(coordinator.uiState().currentOverlay == null) { "overlay still present after response ack" }
check(coordinator.uiState().activeSessions.size == 1) { "expected one active session after response ack" }
check(openSessionCalls.get() >= 1) { "expected at least one opened session" }

val completedSnapshot = coordinator.snapshotJson()
val restored = WalletAbiWalletConnectCoordinator.fromSnapshotJson(
    provider,
    "wallet-1",
    completedSnapshot,
)
val completedReconcile = restored.reconcilePendingRequests(listOf(requestRecord))
check(completedReconcile.actions.size == 1) { "expected one cached replay action, got ${completedReconcile.actions.size}" }
check(completedReconcile.requestsToReplay.isEmpty()) { "completed reconcile unexpectedly asked for replay" }
