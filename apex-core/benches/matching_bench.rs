mod common;
use apex_core::prelude::*;
use common::*;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rand::Rng;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

fn bench_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("matching for 10k orders");
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("match_orders 10k", |b| {
        let syncer = Arc::new(EmptyOrderBookSyncer {});
        let id = Arc::new(AtomicU64::new(1));
        let book = Arc::new(DefaultOrderBook::new(id, syncer));
        let engine = DefaultMatchingEngine::new(book);

        // Insert 10_000 sell orders
        for i in 0..10_000 {
            let mut sell = make_limit_order(i, Side::Sell, 1000 + (i % 500), 10, 3000 + i);
            engine.create_order(&mut sell).unwrap();
        }

        // Insert 10_000 buy orders
        for i in 10_000..20_000 {
            let mut buy = make_limit_order(i, Side::Buy, 1500, 10, 4000 + i);
            engine.create_order(&mut buy).unwrap();
        }

        b.iter(|| {
            engine.match_orders();
        });
    });
    group.finish();
}

fn stress_multi_thread_benchmark(c: &mut Criterion) {
    let syncer: Arc<dyn OrderBookSyncer> = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = Arc::new(DefaultMatchingEngine::new(book));

    let insert_counter = Arc::new(AtomicU64::new(0));
    let cancel_counter = Arc::new(AtomicU64::new(0));

    let mut group = c.benchmark_group("stress matching");
    group.throughput(Throughput::Elements(1));
    group.bench_function("multi-thread insert/cancel/match TPS", |b| {
        let running = Arc::new(AtomicBool::new(true));

        // Start insert thread
        let engine_insert = Arc::clone(&engine);
        let insert_counter_clone = Arc::clone(&insert_counter);
        let insert_thread_running = running.clone();
        let insert_thread = thread::spawn(move || {
            let mut i = 0u64;
            while insert_thread_running.load(Ordering::Relaxed) {
                i += 1;
                let mut rng = rand::rng();
                let order_type = rng.random_bool(0.3); // 30% Limit, 70% Market
                let is_buy = rng.random_bool(0.5); // 50% Buy, 50% Sell
                let side = if is_buy { Side::Buy } else { Side::Sell };
                if order_type {
                    let mut order = make_limit_order(i, side, 1000 - (i % 500), 10, 1000 + i);
                    let _ = engine_insert.create_order(&mut order);
                } else {
                    let mut order = make_market_order(i, side, 10, 2000 + i);
                    let _ = engine_insert.create_order(&mut order);
                }
                insert_counter_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        // Start cancel thread
        let engine_cancel = Arc::clone(&engine);
        let cancel_counter_clone = Arc::clone(&cancel_counter);
        let cancel_thread_running = running.clone();
        let cancel_thread = thread::spawn(move || {
            let mut rng = rand::rng();
            while cancel_thread_running.load(Ordering::Relaxed) {
                let random_id = rng.random_range(0..100_000_000);
                let _ = engine_cancel.cancel_order(random_id);
                cancel_counter_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        b.iter(|| {
            engine.match_orders();
        });

        running.store(false, Ordering::Relaxed);
        insert_thread.join().unwrap();
        cancel_thread.join().unwrap();
    });
    group.finish();
}

criterion_group!(benches, bench_matching, stress_multi_thread_benchmark);
criterion_main!(benches);
