use crate::prelude::*;

/// OrderBookSyncer trait is used to synchronize the order book with the nodes
pub trait OrderBookSyncer: Send + Sync {
    /// This function is called when the order book accepts a new order
    fn add_order(&self, id: u64, order: &Order);
    /// This function is called when the order book updates an order
    fn update_order(&self, id: u64, order: &Order);
    /// This function is called when the order book cancels an order
    fn cancel_order(&self, id: u64, order: &Order);
    /// This function is called when the order engine matches an order
    fn matched(&self, id: u64, updated: &[Order], trades: &[Trade]);
}

/// EmptyOrderBookSyncer is a no-op implementation of OrderBookSyncer
pub struct EmptyOrderBookSyncer {}

impl OrderBookSyncer for EmptyOrderBookSyncer {
    fn add_order(&self, _id: u64, _order: &Order) {}

    fn update_order(&self, _id: u64, _order: &Order) {}

    fn cancel_order(&self, _id: u64, _order: &Order) {}

    fn matched(&self, _id: u64, _updated: &[Order], _trades: &[Trade]) {}
}
