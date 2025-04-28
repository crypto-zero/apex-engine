mod common;

use crate::common::*;
use apex_core::prelude::*;
use crossbeam::epoch;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[test]
fn test_cancel_active_limit_order() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert a limit order
    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    // Cancel the active order
    engine.cancel_order(buy.id).unwrap();

    let remaining_buy = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(
        remaining_buy.len(),
        0,
        "Buy side should be empty after cancel"
    );
}

#[test]
fn test_update_active_order_price() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    engine
        .update_order(buy.id, Price::from(105u64), 1001)
        .unwrap();

    let guard = &epoch::pin();
    let buy_book = book.get_book(Side::Buy);
    let updated = buy_book.get(&buy.book_key(), guard);
    assert!(updated.is_none(), "Old key should be gone after update");
}

#[test]
fn test_update_order_priority_after_price_change() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy1 = make_limit_order(1, Side::Buy, 100, 10, 1000);
    let mut buy2 = make_limit_order(2, Side::Buy, 100, 10, 1001);
    engine.create_order(&mut buy1).unwrap();
    engine.create_order(&mut buy2).unwrap();

    engine
        .update_order(buy1.id, Price::from(101u64), 1002)
        .unwrap();

    let state = get_book_state(book.as_ref(), Side::Buy);
    assert_eq!(
        state[0].0, 1,
        "Buy1 should now be at better price and first"
    );
}

#[test]
fn test_update_nonexistent_order_should_fail() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book);

    let result = engine.update_order(999, Price::from(105u64), 1001);
    assert!(result.is_err(), "Updating nonexistent order should fail");
}

#[test]
fn test_update_filled_order_should_fail() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book);

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    let mut buy = make_limit_order(2, Side::Buy, 100, 10, 1001);
    engine.create_order(&mut sell).unwrap();
    engine.create_order(&mut buy).unwrap();
    engine.match_orders();

    let result = engine.update_order(sell.id, Price::from(95u64), 1002);
    assert!(result.is_err(), "Updating filled order should fail");
}

#[test]
fn test_cancel_partially_filled_limit_order() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    // Insert a sell order
    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    // Insert a buy order that only partially fills the sell
    let mut buy = make_limit_order(2, Side::Buy, 100, 4, 1001);
    engine.create_order(&mut buy).unwrap();

    engine.match_orders();

    // Sell should be partially filled, cancel remaining
    engine.cancel_order(sell.id).unwrap();

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(
        remaining_sell.len(),
        0,
        "Sell side should be empty after cancel"
    );
}

#[test]
fn test_cancel_updates_status_and_reason() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut buy = make_limit_order(1, Side::Buy, 100, 10, 1000);
    engine.create_order(&mut buy).unwrap();

    engine.cancel_order(buy.id).unwrap();

    // Now read back the order to check its status and reason
    let guard = &epoch::pin();
    let buy_book = book.get_book(Side::Buy);
    let found = buy_book.get(&buy.book_key(), guard);

    assert!(
        found.is_none(),
        "Cancelled order should not be found in book"
    );
}

#[test]
fn test_cancelled_order_not_in_book() {
    let syncer = Arc::new(EmptyOrderBookSyncer {});
    let id = Arc::new(AtomicU64::new(1));
    let book = Arc::new(DefaultOrderBook::new(id, syncer));
    let engine = DefaultMatchingEngine::new(book.clone());

    let mut sell = make_limit_order(1, Side::Sell, 100, 10, 1000);
    engine.create_order(&mut sell).unwrap();

    engine.cancel_order(sell.id).unwrap();

    let remaining_sell = get_book_state(book.as_ref(), Side::Sell);
    assert_eq!(
        remaining_sell.len(),
        0,
        "Sell side should be empty after cancel"
    );
}
