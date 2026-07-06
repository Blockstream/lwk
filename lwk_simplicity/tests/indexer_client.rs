use lwk_simplicity::lending::{IndexerClient, OfferFiltersRequest};

const BASE_URL: &str = "https://lending.dev.blockstream.com/api";

#[tokio::test]
#[ignore = "requires internet connection"]
async fn test_list_offers_default_filters() {
    let c = client();
    let resp = c
        .list_offers(&OfferFiltersRequest::default())
        .await
        .unwrap();
    assert!(resp.total >= resp.items.len() as u64, "total ≥ items count");
    assert!(resp.limit > 0, "default limit should be positive");
    assert_eq!(resp.offset, 0, "default offset should be 0");
}

#[tokio::test]
#[ignore = "requires internet connection"]
async fn test_list_offers_zero_limit() {
    let c = client();
    let filters = OfferFiltersRequest {
        limit: Some(0),
        ..Default::default()
    };
    let resp = c.list_offers(&filters).await.unwrap();
    assert!(resp.items.is_empty());
}

#[tokio::test]
#[ignore = "requires internet connection"]
async fn test_get_factories_by_script_unknown_script() {
    let c = client();
    let factories = c
        .get_factories_by_script("0000000000000000000000000000000000000000")
        .await
        .unwrap();
    assert!(
        factories.is_empty(),
        "unknown script should return empty list"
    );
}

fn client() -> IndexerClient {
    IndexerClient::builder(BASE_URL).build().unwrap()
}
