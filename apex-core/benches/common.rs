use apex_core::prelude::*;
use std::cell::UnsafeCell;

/// Quickly generate a simple limit order for testing
pub fn make_limit_order(id: u64, side: Side, price: u64, qty: u64, ts: u64) -> Order {
    Order {
        id,
        user_id: 1,
        side,
        price: Price::from(price),
        quantity: UnsafeCell::new(Quantity::from(qty)),
        created_at: ts,
        updated_at: ts,
        ..Order::default()
    }
}

/// Quickly generate a market order for testing
pub fn make_market_order(id: u64, side: Side, qty: u64, ts: u64) -> Order {
    let mut value = make_limit_order(id, side, 0, qty, ts);
    value.order_type = OrderType::Market;
    value
}
