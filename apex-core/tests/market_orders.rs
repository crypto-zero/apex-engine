#![feature(integer_atomics)]
mod common;

use crate::common::*;
use apex_core::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU128;

#[test]
fn test_market_order_ioc_full_fill() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_market_order(2, Side::Buy, 10, 1001);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_market_order_ioc_partial_fill_and_cancel() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 5, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_market_order(2, Side::Buy, 10, 1001);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_market_order_ioc_no_fill() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy = make_market_order(1, Side::Buy, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_market_order_fok_full_fill() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_market_order(2, Side::Buy, 10, 1001);
    buy.match_strategy = MatchStrategy::FillOrKill;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_market_order_fok_partial_not_enough_and_cancel() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 5, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_market_order(2, Side::Buy, 10, 1001);
    buy.match_strategy = MatchStrategy::FillOrKill;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(remaining_sell.len(), 1);
}

#[test]
fn test_market_order_slippage_pass() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    let mut buy = make_market_order(2, Side::Buy, 10, 1001);
    buy.slippage_tolerance = Some(SlippageTolerance(5));
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_market_order_slippage_exceeded_cancel() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert two sell orders
    let mut sell1 = make_limit_order(1, Side::Sell, 100, 5, 1000); // Good price
    let mut sell2 = make_limit_order(2, Side::Sell, 120, 10, 1001); // Bad price (beyond slippage)
    engine.create_order(&mut sell1).unwrap();
    engine.create_order(&mut sell2).unwrap();

    // Create a market buy order with tight slippage
    let mut buy = make_market_order(3, Side::Buy, 10, 1002); // Wants 10 units
    buy.slippage_tolerance = Some(SlippageTolerance(10)); // 0.10% slippage allowed
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // Only Sell 1 should be matched, Sell 2 should remain
    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);

    assert_eq!(
        remaining_sell.len(),
        1,
        "Sell2 should remain because slippage was exceeded"
    );
    assert_eq!(
        remaining_sell[0].0, 2,
        "Remaining sell order should be sell2 (id=2)"
    );
    assert_eq!(
        remaining_sell[0].1, 10,
        "Sell2 should have full quantity left"
    );
}

#[test]
fn test_market_order_on_empty_book() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy = make_market_order(1, Side::Buy, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining.len(), 0);
}
