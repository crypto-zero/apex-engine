use crate::prelude::*;
use crypto_bigint::Zero;
use std::sync::Arc;
use std::time::Instant;

/// MatchingEngine is a trait for matching engine
pub trait MatchingEngine {
    /// Creates a new order and then puts it into the order book
    fn create_order(&self, order: &mut Order) -> Result<(), RejectReason>;
    /// Updates an order in the order book
    fn update_order(
        &self,
        order_id: u64,
        new_price: Price,
        now_microseconds: u64,
    ) -> Result<(), UpdateOrderError>;
    /// Cancels an order in the order book
    fn cancel_order(&self, order_id: u64) -> Result<(), CancelOrderError>;
    /// Matches orders in the order book
    fn match_orders(&self);
}

pub struct DefaultMatchingEngine {
    order_book: Arc<dyn OrderBookWalker>,
}

impl DefaultMatchingEngine {
    /// Creates a new matching engine
    pub fn new(order_book: Arc<dyn OrderBookWalker>) -> Self {
        Self { order_book }
    }

    fn process_order_pair(
        taker: &Order,
        maker: &Order,
        updated: &mut Vec<Order>,
        matched: &mut Vec<Trade>,
    ) -> bool {
        let now_microseconds = Instant::now().elapsed().as_micros() as u64;
        let trades = Trade::matched(now_microseconds, taker, maker);

        if trades.is_none() {
            maker.exit_matched();
            return false;
        }

        let cloned_order;
        let removed = maker.is_filled();
        if !removed {
            cloned_order = maker.clone_reset_lifecycle();
            maker.exit_matched();
        } else {
            maker.enter_finished_from_matched();
            cloned_order = maker.clone();
        }
        updated.push(cloned_order);

        let pair = trades.unwrap();
        matched.push(pair.0);
        matched.push(pair.1);
        removed
    }

    fn lock_book_liquidity(
        &self,
        quantity: Quantity,
        slippage_price: Option<Price>,
    ) -> Option<Vec<OrderID>> {
        let mut order_id_list = Vec::new();
        let mut remaining_qty = quantity;
        let mut walking = |maker: &Order| {
            if !maker.enter_matched() {
                return WalkingResult::next();
            }

            remaining_qty = remaining_qty.saturating_sub(&maker.quantity());
            order_id_list.push(maker.id);

            if remaining_qty.is_zero().into() {
                WalkingResult::exit()
            } else {
                WalkingResult::next()
            }
        };

        self.order_book
            .walking_book_maker(Side::Sell, slippage_price, &mut walking);

        if remaining_qty.is_zero().into() {
            return Some(order_id_list);
        }

        self.order_book
            .walking_by_order_id_list(order_id_list.as_slice(), &mut |o| {
                o.exit_matched();
                WalkingResult::next()
            });
        None
    }

    fn match_market_order_fok(
        &self,
        slippage_price: Option<Price>,
        taker: &Order,
    ) -> WalkingResult {
        let (mut updated, mut matched) = (Vec::new(), Vec::new());

        let order_id_list_opt = self.lock_book_liquidity(taker.quantity(), slippage_price);
        if order_id_list_opt.is_none() {
            taker.update_status(OrderStatus::Rejected);
            taker.update_reject_reason(RejectReason::InsufficientLiquidity);
            taker.enter_finished_from_matched();
            updated.push(taker.clone());
            self.order_book.sync_matched(&updated, &matched);
            return WalkingResult::remove_and_next();
        }

        let mut process = |maker: &Order| {
            let removed =
                DefaultMatchingEngine::process_order_pair(taker, maker, &mut updated, &mut matched);
            WalkingResult::new(removed, taker.quantity().is_zero().into())
        };
        self.order_book
            .walking_by_order_id_list(order_id_list_opt.unwrap().as_slice(), &mut process);

        taker.enter_finished_from_matched();
        updated.push(taker.clone());

        self.order_book.sync_matched(&updated, &matched);

        WalkingResult::remove_and_next()
    }

    fn match_market_order(&self, taker: &Order) -> WalkingResult {
        if !taker.enter_matched() {
            return WalkingResult::next();
        }

        let opposite_side = if taker.side == Side::Buy {
            Side::Sell
        } else {
            Side::Buy
        };
        let best_price = self.order_book.get_best_price(opposite_side);
        let slippage_price = match best_price {
            None => None,
            Some(price) => taker.slippage_bound_price(price),
        };

        if taker.match_strategy == MatchStrategy::FillOrKill {
            return self.match_market_order_fok(slippage_price, taker);
        }

        // Process market order as IOC
        let (mut updated, mut matched) = (Vec::new(), Vec::new());
        let mut process = |maker: &Order| {
            if !maker.enter_matched() {
                return WalkingResult::next();
            }
            let removed =
                DefaultMatchingEngine::process_order_pair(taker, maker, &mut updated, &mut matched);
            WalkingResult::new(removed, taker.quantity().is_zero().into())
        };
        self.order_book
            .walking_book_maker(opposite_side, slippage_price, &mut process);

        if matched.is_empty() {
            taker.update_status(OrderStatus::Rejected);
            taker.update_reject_reason(RejectReason::InsufficientLiquidity);
        }
        taker.enter_finished_from_matched();
        updated.push(taker.clone());

        self.order_book.sync_matched(&updated, &matched);

        WalkingResult::remove_and_next()
    }

    fn match_limit_order(&self, taker: &Order) -> WalkingResult {
        if !taker.enter_matched() {
            return WalkingResult::next();
        }

        let opposite_side = if taker.side == Side::Buy {
            Side::Sell
        } else {
            Side::Buy
        };

        let (mut updated, mut matched) = (Vec::new(), Vec::new());
        let mut process = |maker: &Order| {
            if !maker.enter_matched() {
                return WalkingResult::next();
            }
            let removed =
                DefaultMatchingEngine::process_order_pair(taker, maker, &mut updated, &mut matched);
            WalkingResult::new(removed, taker.quantity().is_zero().into())
        };
        self.order_book
            .walking_book_maker(opposite_side, Some(taker.price), &mut process);

        if updated.is_empty() && matched.is_empty() {
            taker.exit_matched();
            return WalkingResult::next();
        }

        let cloned_order;
        let removed = taker.is_filled();
        if !removed {
            cloned_order = taker.clone_reset_lifecycle();
            taker.exit_matched();
        } else {
            taker.enter_finished_from_matched();
            cloned_order = taker.clone();
        }
        updated.push(cloned_order);

        self.order_book.sync_matched(&updated, &matched);

        WalkingResult::new(removed, false)
    }
}

impl MatchingEngine for DefaultMatchingEngine {
    fn create_order(&self, order: &mut Order) -> Result<(), RejectReason> {
        self.order_book.insert(order)
    }

    fn update_order(
        &self,
        order_id: u64,
        new_price: Price,
        now_microseconds: u64,
    ) -> Result<(), UpdateOrderError> {
        self.order_book
            .update_order(order_id, new_price, now_microseconds)
    }

    fn cancel_order(&self, order_id: u64) -> Result<(), CancelOrderError> {
        self.order_book.remove(order_id)
    }

    fn match_orders(&self) {
        let mut walking = |order: &Order| self.match_market_order(order);
        self.order_book.walking_market_book(&mut walking);

        let mut walking = |taker: &Order| self.match_limit_order(taker);
        self.order_book.walking_cross_taker(&mut walking);
    }
}
