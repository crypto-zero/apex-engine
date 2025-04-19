#![feature(integer_atomics)]

use apex_core::prelude::*;
use crossbeam::epoch;
use crossbeam::epoch::default_collector;
use crossbeam_skiplist::SkipList;

/// Quickly generate a simple limit order for testing
pub fn make_limit_order(id: u64, side: Side, price: Price, qty: Quantity, ts: u64) -> Order {
    let mut value = Order::default();
    value.id = id;
    value.user_id = 1;
    value.side = side;
    value.price = price;
    *value.quantity.get_mut() = qty;
    value.created_at = ts;
    value.updated_at = ts;
    value
}

/// Quickly generate a market order for testing
pub fn make_market_order(id: u64, side: Side, qty: Quantity, ts: u64) -> Order {
    let mut value = make_limit_order(id, side, 0, qty, ts);
    value.order_type = OrderType::Market;
    value
}

/// Get the current state of a side of the book
pub fn get_book_state(book: &dyn OrderBookWalker, side: Side) -> Vec<(OrderID, Quantity)> {
    let guard = &epoch::pin();
    book.get_book(side)
        .iter(guard)
        .map(|entry| (entry.value().id, entry.value().quantity()))
        .collect()
}

#[test]
fn test_skiplist_next_when_delete() {
    let list = SkipList::new(default_collector().clone());
    let guard = &epoch::pin();
    let _entry1 = list.get_or_insert(1, 1, guard);
    let entry2 = list.get_or_insert(2, 2, guard);
    let _entry3 = list.get_or_insert(3, 3, guard);

    let front = list.front(guard).unwrap();
    entry2.remove(guard);
    let next = front.next().unwrap();
    let tail = next.next();

    assert_eq!(front.key(), &1);
    assert_eq!(next.key(), &3);
    assert!(tail.is_none());
}
