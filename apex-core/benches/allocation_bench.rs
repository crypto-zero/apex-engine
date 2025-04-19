use apex_core::engine::prelude::*;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::thread;

fn bench_alloc_dealloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool alloc");
    group.throughput(Throughput::Elements(1));
    group.bench_function("alloc + drop", |b| {
        b.iter(|| {
            let order = black_box(Box::new(Order::default()));
            drop(order);
        });
    });
    group.finish();
}

fn bench_concurrent_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent alloc");
    group.throughput(Throughput::Elements(1000));
    group.bench_function("order_pool concurrent alloc", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..8 {
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        let order = black_box(Box::new(Order::default()));
                        drop(order);
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });
}

fn bench_pool_expansion(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool expansion");
    group.throughput(Throughput::Elements(100000));
    group.bench_function("order_pool expansion (multi-page)", |b| {
        b.iter(|| {
            let mut orders = vec![];
            for _ in 0..100000 {
                let order = black_box(Box::new(Order::default()));
                orders.push(order);
            }
            drop(orders); // 全部 drop
        });
    });
}

criterion_group!(
    benches,
    bench_alloc_dealloc,
    bench_concurrent_alloc,
    bench_pool_expansion
);
criterion_main!(benches);
