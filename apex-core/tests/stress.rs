mod common;

use crate::common::*;
use apex_core::prelude::*;
use rand::Rng;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[test]
fn test_massive_order_insertion() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    for i in 0..100_000 {
        let mut order = make_limit_order(i, Side::Buy, 1000 - (i % 1000), 10, 1000 + i);
        engine.create_order(&mut order).unwrap();
    }

    let buy_book_state = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(buy_book_state.len(), 100_000);
}

#[test]
fn test_massive_order_cancellation() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book);

    for i in 0..50_000 {
        let mut order = make_limit_order(i, Side::Sell, 1000 + (i % 500), 10, 2000 + i);
        engine.create_order(&mut order).unwrap();
    }

    // Randomly cancel half of them
    let mut rng = rand::rng();
    for _i in 0..25_000 {
        let id_to_cancel = rng.random_range(0..50_000);
        let _ = engine.cancel_order(id_to_cancel);
    }

    // No assertion: just ensure no panic
}

#[test]
fn test_massive_order_matching() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book);

    // Insert many sell orders
    for i in 0..30_000 {
        let mut sell = make_limit_order(i, Side::Sell, 1000 + (i % 500), 10, 3000 + i);
        engine.create_order(&mut sell).unwrap();
    }

    // Insert many buy orders that will aggressively cross sell orders
    for i in 30_000..60_000 {
        let mut buy = make_limit_order(i, Side::Buy, 2000, 10, 4000 + i);
        engine.create_order(&mut buy).unwrap();
    }

    // Trigger matching
    engine.match_orders();

    // Verify no panic during matching
}
