mod common;

use crate::common::*;
use apex_core::prelude::*;
use crossbeam::epoch;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[test]
fn test_limit_order_full_fill_removal() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_limit_order(2, Side::Buy, 100, 10, 1001);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(
        remaining.len(),
        0,
        "Sell order should be fully filled and removed"
    );
}

#[test]
fn test_limit_order_priority_by_time() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell1 = make_limit_order(1, Side::Sell, 100, 10, 1000); // Earlier
    let mut sell2 = make_limit_order(2, Side::Sell, 100, 10, 1005); // Later
    engine.create_order(&mut sell1).unwrap();
    engine.create_order(&mut sell2).unwrap();

    let mut buy = make_limit_order(3, Side::Buy, 100, 10, 1010);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].0, 2,
        "Sell2 should remain because Sell1 was matched first"
    );
}

#[test]
fn test_limit_order_no_cross_no_fill() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 105, 10, 1000); // Higher price
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_limit_order(2, Side::Buy, 100, 10, 1001); // Lower price
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);
    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);

    assert_eq!(
        remaining_sell.len(),
        1,
        "Sell should stay because price is too high"
    );
    assert_eq!(
        remaining_buy.len(),
        1,
        "Buy should stay because price is too low"
    );
}

#[test]
fn test_limit_order_multiple_partial_fills() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert multiple sell orders
    let mut sell1 = make_limit_order(1, Side::Sell, 100, 5, 1000);
    let mut sell2 = make_limit_order(2, Side::Sell, 100, 5, 1001);
    engine.create_order(&mut sell1).unwrap();
    engine.create_order(&mut sell2).unwrap();

    // Insert one big buy order
    let mut buy = make_limit_order(3, Side::Buy, 100, 8, 1002);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);

    assert_eq!(
        remaining_sell.len(),
        1,
        "One partially remaining sell order expected"
    );
    assert_eq!(
        remaining_sell[0],
        (2, 2),
        "Sell2 should have 2 remaining units"
    );
}

#[test]
fn test_limit_order_partial_then_cancel() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_limit_order(2, Side::Buy, 100, 4, 1001);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // After partial match, cancel remaining sell order
    {
        let guard = &epoch::pin();
        let sell_book = book.get_book(Side::Sell);
        if let Some(entry) = sell_book.front(guard) {
            engine.cancel_order(entry.value().id).unwrap();
        }
    }

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(
        remaining_sell.len(),
        0,
        "Remaining sell order should be cancelled"
    );
}

#[test]
fn test_limit_order_partial_and_full_match() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert sell orders
    let mut sell1 = make_limit_order(101, Side::Sell, 100, 10, 1000);
    let mut sell2 = make_limit_order(102, Side::Sell, 100, 10, 1001);
    engine.create_order(&mut sell1).unwrap();
    engine.create_order(&mut sell2).unwrap();

    // Insert buy order - only partially match sell1
    let mut buy1 = make_limit_order(200, Side::Buy, 100, 6, 1002);
    engine.create_order(&mut buy1).unwrap();

    // Process match
    engine.match_orders();

    // Check order book
    let guard = &epoch::pin();
    let sell_book = book.get_book(Side::Sell);

    let remaining: Vec<_> = sell_book
        .iter(guard)
        .map(|entry| (entry.value().id, entry.value().quantity()))
        .collect();

    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0], (101, 4));
    assert_eq!(remaining[1], (102, 10));
}

#[test]
fn test_limit_order_iter_continues_after_remove() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert 3 sell orders
    let mut sell1 = make_limit_order(101, Side::Sell, 100, 10, 1000);
    let mut sell2 = make_limit_order(102, Side::Sell, 100, 10, 1001);
    let mut sell3 = make_limit_order(103, Side::Sell, 100, 10, 1002);
    engine.create_order(&mut sell1).unwrap();
    engine.create_order(&mut sell2).unwrap();
    engine.create_order(&mut sell3).unwrap();

    // Insert 2 buys order that matches with sell1、sell2 partially
    let mut buy1 = make_limit_order(200, Side::Buy, 100, 11, 990);
    let mut buy2 = make_limit_order(201, Side::Buy, 100, 4, 991);
    engine.create_order(&mut buy1).unwrap();
    engine.create_order(&mut buy2).unwrap();

    // Process the match
    engine.match_orders();

    let guard = &epoch::pin();
    let sell_book = book.get_book(Side::Sell);

    let remaining: Vec<_> = sell_book
        .iter(guard)
        .map(|entry| (entry.value().id, entry.value().quantity()))
        .collect();

    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0], (102, 5));
    assert_eq!(remaining[1], (103, 10));
}
