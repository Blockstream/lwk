use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use lwk_test_util::*;
use lwk_wollet::*;

/// Authenticated Explorer API: one call through the real auth path (Keycloak-issued
/// `client_credentials` token, validated by the APISIX `openid-connect` plugin, proxied to the
/// regtest Esplora). Also proves the gateway rejects unauthenticated calls.
#[cfg(feature = "esplora")]
#[tokio::test]
#[ignore = "requires docker and the blockstream/apisix image"]
async fn test_esplora_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_esplora()
        .with_auth()
        .build();
    let gateway_url = env.esplora_url();
    let network = Network::default_regtest();

    // without a token the gateway rejects the call
    let mut client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .build()
        .unwrap();
    assert!(client.tip().await.is_err());

    // with the token provider the token is fetched from keycloak and the call is served
    let token_provider = clients::TokenProvider::Blockstream {
        url: env.oidc_token_url(),
        client_id: lwk_test_util::AUTH_CLIENT_ID.to_string(),
        client_secret: lwk_test_util::AUTH_CLIENT_SECRET.to_string(),
    };
    let mut client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .token_provider(token_provider)
        .build()
        .unwrap();
    let tip = client.tip().await.unwrap();
    assert_eq!(tip.height, 101);
}

/// Authenticated Waterfalls API through the gateway with the production plugin chain
/// (openid-connect → oidc-identity-extractor → credit-checker): a funded wallet syncs via
/// the waterfalls endpoint with a Keycloak token, and exhausted credits deny the calls
/// (402 Payment Required).
#[cfg(feature = "esplora")]
#[test]
#[ignore = "requires docker and the blockstream/apisix image"]
fn test_waterfalls_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_waterfalls()
        .with_auth()
        .build();
    let network = Network::default_regtest();

    let token_provider = || clients::TokenProvider::Blockstream {
        url: env.oidc_token_url(),
        client_id: lwk_test_util::AUTH_CLIENT_ID.to_string(),
        client_secret: lwk_test_util::AUTH_CLIENT_SECRET.to_string(),
    };
    let client = clients::WaterfallsClientBuilder::new(&env.waterfalls_url(), network)
        .token_provider(token_provider())
        .build_blocking()
        .unwrap();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/<0;1>/*))", view_key, signer.xpub());
    let mut wallet = TestWollet::new(client, &desc);

    // funding syncs the wallet through the authenticated gateway
    wallet.fund_btc(&env);

    // valid token but exhausted credits -> denied with 402 Payment Required
    env.set_credits(0);
    let mut denied_client = clients::WaterfallsClientBuilder::new(&env.waterfalls_url(), network)
        .token_provider(token_provider())
        .build_blocking()
        .unwrap();
    let err = denied_client.full_scan(&wallet.wollet).unwrap_err();
    assert!(
        matches!(err, Error::EsploraHttpError { status: 402, .. }),
        "expected an EsploraHttpError with status 402, got: {err:?}"
    );
}

/// Authenticated Electrum RPC through the electrs-electrum-proxy (in-band JWT validated
/// against Keycloak's JWKS + credit checking): a valid token is served by the regtest
/// electrs, bogus/missing tokens and exhausted credits are denied.
///
/// Uses `TokenProvider::Static`; automatic fetch/refresh for Electrum
/// (`TokenProvider::Blockstream`) is tracked in #373.
#[cfg(feature = "electrum")]
#[test]
#[ignore = "requires docker, the blockstream/apisix image and the rpcproxy image"]
fn test_electrum_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_auth()
        .build();
    let gateway_url = env.electrum_url();

    // The gateway is a localhost proxy, the documented case for allowing a token on tcp://
    let tip_with = |token_provider: clients::TokenProvider| {
        ElectrumClientBuilder::new(&gateway_url)
            .token_provider(token_provider)
            .allow_plaintext_with_token(true)
            .build()
            .and_then(|mut client| client.tip())
    };

    // valid token + credits -> served by the regtest electrs
    let token = env.fetch_oidc_token();
    let tip = tip_with(clients::TokenProvider::Static(token.clone())).unwrap();
    assert_eq!(tip.height, 101);

    // The proxy denies with well-defined JSON-RPC error codes, surfaced today inside the
    // wrapped electrum_client protocol error.
    // TODO(#398): map these (and the esplora 401/402/429) to common lwk error variants so
    // callers don't need transport-specific handling.
    let assert_denied_with = |result: Result<elements::BlockHeader, Error>, code: i64| {
        let err = result.unwrap_err();
        match &err {
            Error::ClientError(electrum_client::Error::Protocol(value)) => assert_eq!(
                value.get("code").and_then(|c| c.as_i64()),
                Some(code),
                "expected a denial with JSON-RPC code {code}, got: {value}"
            ),
            other => panic!("expected a protocol error with code {code}, got: {other:?}"),
        }
    };

    // bogus token -> denied with AUTHENTICATION_REQUIRED
    assert_denied_with(
        tip_with(clients::TokenProvider::Static("not-a-jwt".to_string())),
        -32004,
    );

    // missing token -> denied with AUTHENTICATION_REQUIRED
    assert_denied_with(tip_with(clients::TokenProvider::None), -32004);

    // valid token but exhausted credits -> denied with INSUFFICIENT_CREDITS
    env.set_credits(0);
    assert_denied_with(tip_with(clients::TokenProvider::Static(token)), -32000);
}
