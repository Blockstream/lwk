use std::str::FromStr;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use elements::{pset::PartiallySignedTransaction, Address};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use lwk_wollet::{ElementsNetwork, Update, Wollet, WolletDescriptor};

criterion_group!(benches, wollet, address, pset);
criterion_main!(benches);

pub fn wollet(c: &mut Criterion) {
    c.benchmark_group("wollet")
        .bench_function("descriptor parse", |b: &mut criterion::Bencher<'_>| {
            let desc_str = include_str!("../../lwk_common/test_data/pset_details/descriptor");

            b.iter(|| {
                let d: WolletDescriptor = desc_str.parse().unwrap();
                black_box(d);
            });
        })
        .bench_function("wallet transactions", |b: &mut criterion::Bencher<'_>| {
            let wollet = test_wollet_with_many_transactions();
            b.iter(|| {
                let txs = wollet.transactions().unwrap();
                black_box(txs);
            });
        })
        .bench_function("wallet utxos", |b: &mut criterion::Bencher<'_>| {
            let wollet = test_wollet_with_many_transactions();
            b.iter(|| {
                let txs = wollet.utxos().unwrap();
                black_box(txs);
            });
        })
        .bench_function("wallet txos", |b: &mut criterion::Bencher<'_>| {
            let wollet = test_wollet_with_many_transactions();
            b.iter(|| {
                let txs = wollet.txos().unwrap();
                black_box(txs);
            });
        });
}

pub fn address(c: &mut Criterion) {
    c.benchmark_group("address")
        .bench_function("derive blinded", |b: &mut criterion::Bencher<'_>| {
            let desc_str = include_str!("../../lwk_common/test_data/pset_details/descriptor");
            let d: WolletDescriptor = desc_str.parse().unwrap();

            b.iter(|| {
                let address = d
                    .address(0, ElementsNetwork::LiquidTestnet.address_params())
                    .unwrap();
                black_box(address);
            });
        })
        .bench_function("derive unblinded", |b: &mut criterion::Bencher<'_>| {
            let desc_str = include_str!("../../lwk_common/test_data/pset_details/descriptor");
            let d: WolletDescriptor = desc_str.parse().unwrap();
            let d = d
                .descriptor()
                .unwrap()
                .clone()
                .into_single_descriptors()
                .unwrap()
                .pop()
                .unwrap();

            b.iter(|| {
                let address = d
                    .at_derivation_index(0)
                    .unwrap()
                    .address(ElementsNetwork::LiquidTestnet.address_params())
                    .unwrap();
                black_box(address);
            });
        })
        .bench_function("from components", |b: &mut criterion::Bencher<'_>| {
            const ADDR: &str = "lq1qqf8er278e6nyvuwtgf39e6ewvdcnjupn9a86rzpx655y5lhkt0walu3djf9cklkxd3ryld97hu8h3xepw7sh2rlu7q45dcew5";
            let addr = Address::from_str(ADDR).unwrap();
            b.iter(|| {
                let address = Address::from_script(
                    &addr.script_pubkey(),
                    addr.blinding_pubkey,
                    ElementsNetwork::LiquidTestnet.address_params(),
                );
                black_box(address);
            });
        });
}

pub fn pset(c: &mut Criterion) {
    c.benchmark_group("pset")
        .bench_function("pset_balance", |b: &mut criterion::Bencher<'_>| {
            let desc_str = include_str!("../../lwk_common/test_data/pset_details/descriptor");
            let desc: ConfidentialDescriptor<DescriptorPublicKey> = desc_str.parse().unwrap();
            let pset_str = include_str!("../../lwk_common/test_data/pset_details/pset.base64");
            let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
            b.iter(|| {
                let balance = lwk_common::pset_balance(
                    &pset,
                    &desc,
                    &elements::AddressParams::LIQUID_TESTNET,
                )
                .unwrap();

                black_box(balance);
            });
        });
}

// duplicated from tests/test_wollet.rs
pub fn test_wollet_with_many_transactions() -> Wollet {
    let update = lwk_test_util::update_test_vector_many_transactions();
    let descriptor = lwk_test_util::wollet_descriptor_many_transactions();
    let descriptor: WolletDescriptor = descriptor.parse().unwrap();
    let update = Update::deserialize(&update).unwrap();
    let mut wollet = Wollet::without_persist(ElementsNetwork::LiquidTestnet, descriptor).unwrap();
    wollet.apply_update(update).unwrap();
    wollet
}
