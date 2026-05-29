package main

import (
	"log"
	"lwk-go/lwk"
)

func main() {
	node := lwk.NewLwkTestEnv() // launch electrs and elementsd

	// (maker) Create a new wallet with LBTC
	mnemonicStr := "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
	mnemonic, err := lwk.NewMnemonic(mnemonicStr)
	if err != nil {
		log.Fatal(err)
	}

	network := node.Network()
	policyAsset := network.PolicyAsset()
	client, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		log.Fatal(err)
	}

	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		log.Fatal(err)
	}

	desc, err := signer.WpkhSlip77Descriptor()
	if err != nil {
		log.Fatal(err)
	}

	maker, err := lwk.NewWollet(network, desc, nil)
	if err != nil {
		log.Fatal(err)
	}

	utxoSize := uint64(100_000)

	idx0 := uint32(0)
	makerAddrRes, err := maker.Address(&idx0)
	if err != nil {
		log.Fatal(err)
	}

	txid := node.SendToAddress(makerAddrRes.Address(), utxoSize, nil)
	_, err = maker.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	makerBal, _ := maker.Balance()
	if makerBal[policyAsset] != uint64(utxoSize) {
		panic("maker balance mismatch")
	}

	// (taker) Create a second wallet with an issued asset and some LBTC required for the fee (will be the liquidex taker)
	mnem2, err := lwk.MnemonicFromRandom(12)
	if err != nil {
		log.Fatal(err)
	}
	signer2, err := lwk.NewSigner(mnem2, network)
	if err != nil {
		log.Fatal(err)
	}
	desc2, err := signer2.WpkhSlip77Descriptor()
	if err != nil {
		log.Fatal(err)
	}
	taker, err := lwk.NewWollet(network, desc2, nil)
	if err != nil {
		log.Fatal(err)
	}

	idx1 := uint32(1)
	takerAddrRes, err := taker.Address(&idx1)
	if err != nil {
		log.Fatal(err)
	}
	address2 := takerAddrRes.Address()

	issuedAssetUnits := uint64(100)
	asset := node.IssueAsset(issuedAssetUnits)

	txid2 := node.SendToAddress(address2, issuedAssetUnits, &asset)
	txidFee := node.SendToAddress(address2, 1000, nil)

	_, err = taker.WaitForTx(txid2, client)
	if err != nil {
		log.Fatal(err)
	}
	_, err = taker.WaitForTx(txidFee, client)
	if err != nil {
		log.Fatal(err)
	}

	takerBal, _ := taker.Balance()
	if takerBal[asset] != uint64(issuedAssetUnits) {
		panic("taker issued asset balance mismatch")
	}

	// ANCHOR: liquidex_make
	// (maker) Create a liquidex proposal (asking for the issued asset in exchange for the policy asset)
	builder := network.TxBuilder()

	makerUtxos, err := maker.Utxos()
	if err != nil {
		log.Fatal(err)
	}
	if len(makerUtxos) == 0 {
		panic("expected maker utxos")
	}
	utxo := makerUtxos[0].Outpoint()

	makerAddrResNil, err := maker.Address(nil)
	if err != nil {
		log.Fatal(err)
	}

	if err := builder.LiquidexMake(utxo, makerAddrResNil.Address(), issuedAssetUnits, asset); err != nil {
		log.Fatal(err)
	}

	pset, err := builder.Finish(maker)
	if err != nil {
		log.Fatal(err)
	}

	inputs := pset.Inputs()
	if inputs[0].Sighash() != 131 { // ANCHOR: ignore
		panic("expected sighash 131") // ANCHOR: ignore
	} // ANCHOR: ignore

	signedPset, err := signer.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	// (maker) Create the proposal and convert it to string to pass it to the taker
	proposal, err := lwk.UnvalidatedLiquidexProposalFromPset(signedPset)
	if err != nil {
		log.Fatal(err)
	}
	proposalStr := proposal.String()
	// ANCHOR_END: liquidex_make

	// ANCHOR: liquidex_validate
	// (taker) Parse the proposal from string and validate it
	proposalFromStr, err := lwk.NewUnvalidatedLiquidexProposal(proposalStr)
	if err != nil {
		log.Fatal(err)
	}
	neededTxid, err := proposalFromStr.NeededTx()
	if err != nil {
		log.Fatal(err)
	}

	previousTx, err := client.GetTx(neededTxid)
	if err != nil {
		log.Fatal(err)
	}
	validatedProposal, err := proposalFromStr.Validate(previousTx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: liquidex_validate

	// (taker) Verify proposal details
	inputAmount := validatedProposal.Input().Amount()
	outputAmount := validatedProposal.Output().Amount()
	inputAsset := validatedProposal.Input().Asset()
	outputAsset := validatedProposal.Output().Asset()

	if inputAmount != utxoSize {
		panic("unexpected input amount")
	}
	if outputAmount != issuedAssetUnits {
		panic("unexpected output amount")
	}
	if inputAsset != policyAsset {
		panic("unexpected input asset")
	}
	if outputAsset != asset {
		panic("unexpected output asset")
	}

	// ANCHOR: liquidex_take
	// (taker) Accept the proposal
	builder2 := network.TxBuilder()
	if err := builder2.LiquidexTake([]*lwk.ValidatedLiquidexProposal{validatedProposal}); err != nil {
		log.Fatal(err)
	}
	pset2, err := builder2.Finish(taker)
	if err != nil {
		log.Fatal(err)
	}

	inputs2 := pset2.Inputs()
	if inputs2[1].Sighash() != 1 { // ANCHOR: ignore
		panic("expected sighash 1") // ANCHOR: ignore
	} // ANCHOR: ignore

	signedPset2, err := signer2.Sign(pset2)
	if err != nil {
		log.Fatal(err)
	}

	// (taker) Finalize and broadcast the transaction
	finalizedPset, err := taker.Finalize(signedPset2)
	if err != nil {
		log.Fatal(err)
	}

	tx, err := finalizedPset.ExtractTx()
	if err != nil {
		log.Fatal(err)
	}

	txid3, err := client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: liquidex_take

	_, err = maker.WaitForTx(txid3, client)
	if err != nil {
		log.Fatal(err)
	}
	_, err = taker.WaitForTx(txid3, client)
	if err != nil {
		log.Fatal(err)
	}

	// Verify the final balance
	finalMakerBal, _ := maker.Balance()
	if finalMakerBal[asset] != uint64(issuedAssetUnits) {
		panic("maker did not receive the asset")
	}

	finalTakerBal, _ := taker.Balance()
	if finalTakerBal[asset] != 0 {
		panic("taker did not spend the asset")
	}
}
