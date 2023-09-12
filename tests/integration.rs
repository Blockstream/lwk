mod test_session;

use test_session::*;

#[test]
fn liquid() {
    let mut server = setup();
    let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
    let master_blinding_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", master_blinding_key, xpub);
    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&mut server);
    let _asset = wallet.fund_asset(&mut server);
}

#[test]
fn view() {
    let mut server = setup();
    // "view" descriptor
    let xpub = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
    let descriptor_blinding_key = "tprv8ZgxMBicQKsPd7qLuJ7yJhzbwSrNfh9MF5qR4tJRPCs63xksUdTAF79dUHADNygu5kLTsXC6jtq4Cibsy6QCVBEboRzAH48vw5zoLkJTuso";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&mut server);
    let _asset = wallet.fund_asset(&mut server);
}
