use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lwk_wollet::WolletDescriptor;

criterion_group!(benches, descriptor);
criterion_main!(benches);

pub fn descriptor(c: &mut Criterion) {
    c.benchmark_group("descriptor")
        .bench_function("parse", |b: &mut criterion::Bencher<'_>| {
            let desc_str = include_str!("../../lwk_common/test_data/pset_details/descriptor");

            b.iter(|| {
                let d: WolletDescriptor = desc_str.parse().unwrap();
                black_box(d);
            });
        });
}
