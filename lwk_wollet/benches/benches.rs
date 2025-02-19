use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lwk_wollet::{ElementsNetwork, NoPersist, Update, Wollet, WolletDescriptor};

criterion_group!(benches, wollet);
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

// duplicated from tests/test_wollet.rs
pub fn test_wollet_with_many_transactions() -> Wollet {
    let update = lwk_test_util::update_test_vector_many_transactions();
    let descriptor = lwk_test_util::wollet_descriptor_many_transactions();
    let descriptor: WolletDescriptor = descriptor.parse().unwrap();
    let update = Update::deserialize(&update).unwrap();
    let mut wollet = Wollet::new(
        ElementsNetwork::LiquidTestnet,
        std::sync::Arc::new(NoPersist {}),
        descriptor,
    )
    .unwrap();
    wollet.apply_update(update).unwrap();
    wollet
}
