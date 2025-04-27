mod common;

use crate::common::*;
use apex_core::prelude::*;
use crossbeam::epoch;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[test]
fn test_lifecycle_initial_state_active() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book);

    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    assert_eq!(
        OrderLifecycle::from(buy.lifecycle.load(std::sync::atomic::Ordering::Acquire)),
        OrderLifecycle::Active
    );
}

#[test]
fn test_lifecycle_transition_to_matched() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    let mut buy = make_limit_order(2, Side::Buy, 100, 10, 1001);
    engine.create_order(&mut sell).unwrap();
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // Now get the real entries from the book
    let guard = &epoch::pin();
    let sell_book = book.get_book(Side::Sell);
    let buy_book = book.get_book(Side::Buy);

    // They should be gone from book after full fill
    assert!(
        sell_book.get(&sell.book_key(), guard).is_none(),
        "Sell should be removed after match"
    );
    assert!(
        buy_book.get(&buy.book_key(), guard).is_none(),
        "Buy should be removed after match"
    );
}

#[test]
fn test_lifecycle_transition_to_finished_after_full_fill() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 5, 1000);
    let mut buy = make_limit_order(2, Side::Buy, 100, 5, 1001);
    engine.create_order(&mut sell).unwrap();
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let guard = &epoch::pin();
    let sell_book = book.get_book(Side::Sell);
    let buy_book = book.get_book(Side::Buy);

    assert!(
        sell_book.get(&sell.book_key(), guard).is_none(),
        "Sell order should be removed after full fill"
    );
    assert!(
        buy_book.get(&buy.book_key(), guard).is_none(),
        "Buy order should be removed after full fill"
    );
}

#[test]
fn test_lifecycle_transition_to_finished_after_cancel() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    engine.cancel_order(buy.id).unwrap();

    let guard = &epoch::pin();
    let buy_book = book.get_book(Side::Buy);
    let found = buy_book.get(&buy.book_key(), guard);

    assert!(
        found.is_none(),
        "Order should be removed from book after cancel"
    );
}
