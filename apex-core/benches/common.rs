use apex_core::prelude::*;

/// Quickly generate a simple limit order for testing
pub fn make_limit_order(id: u64, side: Side, price: u64, qty: u64, ts: u64) -> Order {
    let mut value = Order::default();
    value.id = id;
    value.user_id = 1;
    value.side = side;
    value.price = Price::from(price);
    *value.quantity.get_mut() = Quantity::from(qty);
    value.created_at = ts;
    value.updated_at = ts;
    value
}

/// Quickly generate a market order for testing
pub fn make_market_order(id: u64, side: Side, qty: u64, ts: u64) -> Order {
    let mut value = make_limit_order(id, side, 0, qty, ts);
    value.order_type = OrderType::Market;
    value
}
