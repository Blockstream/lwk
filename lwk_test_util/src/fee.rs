use elements_miniscript::elements::pset::PartiallySignedTransaction;

const DEFAULT_FEE_RATE: f32 = 100.0;

pub fn compute_fee_rate_without_discount_ct(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn compute_fee_rate(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().discount_vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn assert_fee_rate(fee_rate: f32, expected: Option<f32>) {
    let expected = expected.unwrap_or(DEFAULT_FEE_RATE);
    let toll = 0.45;
    assert!(fee_rate > expected * (1.0 - toll));
    assert!(fee_rate < expected * (1.0 + toll));
}
