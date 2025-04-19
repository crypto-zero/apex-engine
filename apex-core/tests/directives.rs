#![feature(integer_atomics)]
mod common;

use crate::common::*;
use apex_core::prelude::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU128;

#[test]
fn test_maker_only_skip_match_when_cross() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert a sell order with MakerOnly (important!)
    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    sell.liquidity_directive = LiquidityDirective::MakerOnly;
    engine.create_order(&mut sell).unwrap();

    // Insert a buy order with MakerOnly at price crossing sell
    let mut buy = make_limit_order(2, Side::Buy, 110, 10, 1001);
    buy.liquidity_directive = LiquidityDirective::MakerOnly;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // Buy should not match, should stay in book
    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining_buy.len(), 1);
    assert_eq!(remaining_buy[0].0, 2);
}

#[test]
fn test_maker_only_hangs_when_no_cross() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // No sell orders
    // Insert a buy order with MakerOnly
    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    buy.liquidity_directive = LiquidityDirective::MakerOnly;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining_buy.len(), 1);
    assert_eq!(remaining_buy[0].0, 1);
}

#[test]
fn test_taker_only_match_when_cross() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert a sell order with AllowTaker
    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    sell.liquidity_directive = LiquidityDirective::AllowTaker;
    engine.create_order(&mut sell).unwrap();

    // Insert a buy order with TakerOnly
    let mut buy = make_limit_order(2, Side::Buy, 110, 10, 1001);
    buy.liquidity_directive = LiquidityDirective::TakerOnly;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // Both buy and sell orders should be gone after match
    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);
    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);

    assert_eq!(
        remaining_buy.len(),
        0,
        "Buy side should be empty after match"
    );
    assert_eq!(
        remaining_sell.len(),
        0,
        "Sell side should be empty after match"
    );
}

#[test]
fn test_taker_only_stays_when_no_cross() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU128::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // No sell orders
    // Insert a buy order with TakerOnly
    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    buy.liquidity_directive = LiquidityDirective::TakerOnly;
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(remaining_buy.len(), 1);
    assert_eq!(remaining_buy[0].0, 1);
}
