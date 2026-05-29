package main

import (
	"fmt"
	"log"

	"lwk-go/lwk"
)

func main() {
	// Start nodes
	node := lwk.NewLwkTestEnv()

	// ANCHOR: test_issue_asset
	network := lwk.NetworkTestnet()
	network = node.Network() // ANCHOR: ignore
	policyAsset := network.PolicyAsset()

	client, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		log.Fatal(err)
	}

	// Create wallet
	mnemonicStr := "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
	mnemonic, err := lwk.NewMnemonic(mnemonicStr)
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
	if desc.String() != "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d" { // ANCHOR: ignore
		panic("descriptor mismatch") // ANCHOR: ignore
	} // ANCHOR: ignore

	wollet, err := lwk.NewWollet(network, desc, nil)
	if err != nil {
		log.Fatal(err)
	}

	idx := uint32(0)
	wolletAddressResult, err := wollet.Address(&idx)
	if err != nil {
		log.Fatal(err)
	}
	if wolletAddressResult.Index() != 0 { // ANCHOR: ignore
		panic("address index mismatch") // ANCHOR: ignore
	} // ANCHOR: ignore

	wolletAddress := wolletAddressResult.Address()
	if fmt.Sprintf("%v", wolletAddress) != "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq" { // ANCHOR: ignore
		panic("address mismatch") // ANCHOR: ignore
	} // ANCHOR: ignore
	fundedSatoshi := uint64(100000)                               // ANCHOR: ignore
	txid := node.SendToAddress(wolletAddress, fundedSatoshi, nil) // ANCHOR: ignore
	_, err = wollet.WaitForTx(txid, client)                       // ANCHOR: ignore
	if err != nil {                                               // ANCHOR: ignore
		log.Fatal(err) // ANCHOR: ignore
	} // ANCHOR: ignore
	balance, err := wollet.Balance() // ANCHOR: ignore
	if err != nil {                  // ANCHOR: ignore
		log.Fatal(err) // ANCHOR: ignore
	} // ANCHOR: ignore
	if balance[policyAsset] != uint64(fundedSatoshi) { // ANCHOR: ignore
		panic("funding balance mismatch") // ANCHOR: ignore
	} // ANCHOR: ignore

	// ANCHOR: contract
	contract, err := lwk.NewContract(
		"ciao.it",
		"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904",
		"name",
		8,
		"TTT",
		0,
	)
	if err != nil {
		log.Fatal(err)
	}
	if contract.String() != `{"entity":{"domain":"ciao.it"},"issuer_pubkey":"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904","name":"name","precision":8,"ticker":"TTT","version":0}` { // ANCHOR: ignore
		panic("contract string mismatch") // ANCHOR: ignore
	} // ANCHOR: ignore
	// ANCHOR_END: contract

	// ANCHOR: issue_asset
	issuedAsset := uint64(10000)
	reissuanceTokens := uint64(1)

	// Create an issuance transaction
	builder := network.TxBuilder()
	if err := builder.IssueAsset(issuedAsset, &wolletAddress, reissuanceTokens, &wolletAddress, &contract); err != nil {
		log.Fatal(err)
	}
	unsignedPset, err := builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: issue_asset

	// Sign the transaction and finalize it
	signedPset, err := signer.Sign(unsignedPset)
	if err != nil {
		log.Fatal(err)
	}

	tx, err := signedPset.Finalize()
	if err != nil {
		log.Fatal(err)
	}

	// Broadcast the transaction
	txid, err = client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}

	// ANCHOR: issuance_ids
	signedInputs := signedPset.Inputs()
	if len(signedInputs) == 0 {
		panic("no inputs on signed pset")
	}
	assetId := signedInputs[0].IssuanceAsset()
	tokenId := signedInputs[0].IssuanceToken()
	// ANCHOR_END: issuance_ids
	// ANCHOR_END: test_issue_asset

	txin := tx.Inputs()[0]

	derivedAssetId, err := lwk.DeriveAssetId(txin, contract)
	if err != nil {
		log.Fatal(err)
	}
	if derivedAssetId != *assetId {
		panic("derived assetId mismatch")
	}

	derivedTokenId, err := lwk.DeriveTokenId(txin, contract)
	if err != nil {
		log.Fatal(err)
	}
	if derivedTokenId != *tokenId {
		panic("derived tokenId mismatch")
	}

	issuance := *unsignedPset.Inputs()[0].Issuance()
	if *issuance.Asset() != *assetId {
		fmt.Println("issuance ", *issuance.Asset())
		fmt.Println("issuance ", *assetId)
		panic("issuance asset mismatch")
	}
	if *issuance.Token() != *tokenId {
		panic("issuance token mismatch")
	}
	if issuance.IsConfidential() {
		panic("expected not confidential")
	}
	if issuance.IsNull() {
		panic("expected not null")
	}
	if !issuance.IsIssuance() {
		panic("expected to be issuance")
	}
	if issuance.IsReissuance() {
		panic("expected not to be reissuance")
	}
	if *issuance.AssetSatoshi() != issuedAsset {
		panic("asset satoshi mismatch")
	}
	if *issuance.TokenSatoshi() != reissuanceTokens {
		panic("token satoshi mismatch")
	}

	_, err = wollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	balance, err = wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}
	if balance[*assetId] != uint64(issuedAsset) {
		panic("issued asset balance mismatch")
	}
	if balance[*tokenId] != uint64(reissuanceTokens) {
		panic("issued token balance mismatch")
	}

	// ANCHOR: reissue_asset
	reissueAsset := uint64(100)
	var assetReceiver **lwk.Address = nil  // Send the asset to the wollet creating the PSET
	var issuanceTx **lwk.Transaction = nil // issuance transaction is present in the same wallet

	builder = network.TxBuilder()
	if err := builder.ReissueAsset(*assetId, reissueAsset, assetReceiver, issuanceTx); err != nil {
		log.Fatal(err)
	}

	unsignedPset, err = builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	signedPset, err = signer.Sign(unsignedPset)
	if err != nil {
		log.Fatal(err)
	}
	tx, err = signedPset.Finalize()
	if err != nil {
		log.Fatal(err)
	}
	txid, err = client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: reissue_asset

	var reissuance *lwk.Issuance
	for _, input := range unsignedPset.Inputs() {
		if input.Issuance() != nil {
			reissuance = *input.Issuance()
			break
		}
	}
	if reissuance == nil {
		panic("reissuance not found")
	}

	if *reissuance.Asset() != *assetId {
		panic("reissuance asset mismatch")
	}
	if *reissuance.Token() != *tokenId {
		panic("reissuance token mismatch")
	}
	if reissuance.IsConfidential() {
		panic("expected not confidential")
	}
	if reissuance.IsNull() {
		panic("expected not null")
	}
	if reissuance.IsIssuance() {
		panic("expected not to be issuance")
	}
	if !reissuance.IsReissuance() {
		panic("expected to be reissuance")
	}
	if *reissuance.AssetSatoshi() != reissueAsset {
		panic("reissuance asset satoshi mismatch")
	}

	_, err = wollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	balance, err = wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}
	if balance[*assetId] != uint64(issuedAsset+reissueAsset) {
		panic("reissued asset balance mismatch")
	}

	// ANCHOR: burn_asset
	burnAsset := uint64(50)

	builder = network.TxBuilder()
	if err := builder.AddBurn(burnAsset, *assetId); err != nil {
		log.Fatal(err)
	}

	unsignedPset, err = builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	signedPset, err = signer.Sign(unsignedPset)
	if err != nil {
		log.Fatal(err)
	}
	tx, err = signedPset.Finalize()
	if err != nil {
		log.Fatal(err)
	}
	txid, err = client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: burn_asset

	_, err = wollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	balance, err = wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}

	expectedFinalBalance := uint64(issuedAsset + reissueAsset - burnAsset)
	if balance[*assetId] != expectedFinalBalance {
		panic("burned asset balance mismatch")
	}
}
