use crate::prelude::*;
use crossbeam::epoch;
use crossbeam::epoch::default_collector;
use crossbeam_skiplist::SkipList;
use flurry::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU128, Ordering};

/// OrderBook is a trait for order book
pub trait OrderBook {
    /// Insert order to the order book
    fn insert(&self, order: &mut Order) -> Result<(), RejectReason>;
    /// Update order in the order book
    fn update_order(
        &self,
        order_id: u64,
        new_price: Price,
        now_microseconds: u64,
    ) -> Result<(), UpdateOrderError>;
    /// Remove an order from the order book
    fn remove(&self, order_id: u64) -> Result<(), CancelOrderError>;
    /// Get the best price for a side
    fn get_best_price(&self, side: Side) -> Option<Price>;
    /// Get the book
    fn get_book(&self, side: Side) -> &SkipList<BookKey, Order>;
    /// Sync orders that's matched and trades
    fn sync_matched(&self, updated: &Vec<Order>, trades: &Vec<Trade>);
}

/// WalkingResult is used for match engine walking results
pub struct WalkingResult {
    pub remove: bool,
    pub exit: bool,
}

impl WalkingResult {
    /// Creates a new walking result
    pub fn new(remove: bool, exit: bool) -> Self {
        Self { remove, exit }
    }

    /// Creates a continue walking result
    pub fn next() -> Self {
        Self {
            remove: false,
            exit: false,
        }
    }

    /// Creates a remove entry and continue walking result
    pub fn remove_and_next() -> Self {
        Self {
            remove: true,
            exit: false,
        }
    }

    /// Creates a exit walking result
    pub fn exit() -> Self {
        Self {
            remove: false,
            exit: true,
        }
    }

    /// Creates a remove entry and exit walking result
    pub fn remove_and_exit() -> Self {
        Self {
            remove: true,
            exit: true,
        }
    }
}

/// MatchingEngineWalker trait is used to walk the order book
pub trait MatchingEngineWalker {
    /// Walks the market orders and calls the callback function for each order
    fn walking_market_book(&self, walk: &mut dyn FnMut(&Order) -> WalkingResult);

    /// Walks the order book and calls the callback function for each order whose is that maker
    fn walking_book_maker(
        &self,
        side: Side,
        slip_price_option: Option<Price>,
        walk: &mut dyn FnMut(&Order) -> WalkingResult,
    );

    /// Walks the order book and call the callback function for each order whose is that maker and cross opposite best price
    fn walking_cross_taker(&self, walk: &mut dyn FnMut(&Order) -> WalkingResult);

    /// Walks the order book by order id hash set
    fn walking_by_order_id_list(
        &self,
        order_id_list: &[OrderID],
        walk: &mut dyn FnMut(&Order) -> WalkingResult,
    );
}

/// OrderBookWalker trait is used to walk the order book
pub trait OrderBookWalker: Send + Sync + OrderBook + MatchingEngineWalker {}

/// DefaultOrderBook is the default implementation of the order book
pub struct DefaultOrderBook {
    id: Arc<AtomicU128>,
    syncer: Arc<dyn OrderBookSyncer>,
    // By order time in microseconds
    market_orders: SkipList<Priority, Order>,
    // By price and then by order time in microseconds
    buy_orders: SkipList<BookKey, Order>,
    // By price and then by order time in microseconds
    sell_orders: SkipList<BookKey, Order>,
    // By order id for fast access order
    order_index: HashMap<OrderID, BookKey>,
}

impl DefaultOrderBook {
    /// Creates a new order book
    pub fn new(id: Arc<AtomicU128>, syncer: Arc<dyn OrderBookSyncer>) -> Self {
        let collector = default_collector().clone();
        let market_orders = SkipList::new(collector.clone());
        let buy_orders = SkipList::new(collector.clone());
        let sell_orders = SkipList::new(collector.clone());
        Self {
            id,
            syncer,
            market_orders,
            buy_orders,
            sell_orders,
            order_index: HashMap::new(),
        }
    }
}

impl OrderBook for DefaultOrderBook {
    /// Insert order into the order book
    fn insert(&self, order: &mut Order) -> Result<(), RejectReason> {
        let guard = &epoch::pin();
        let order_index = self.order_index.pin();

        let book_key = order.book_key();
        match order.order_type {
            OrderType::Limit => {
                let book = match order.side {
                    Side::Buy => &self.buy_orders,
                    Side::Sell => &self.sell_orders,
                };

                order.update_status(OrderStatus::Placed);
                book.get_or_insert(book_key, order.clone(), guard);
            }
            OrderType::Market => {
                order.update_status(OrderStatus::Placed);
                self.market_orders
                    .get_or_insert(order.priority(), order.clone(), guard);
            }
        };
        order_index.insert(order.id, book_key);
        let id = self.id.fetch_add(1, Ordering::Acquire);
        self.syncer.add_order(id, order);

        Ok(())
    }

    /// Updates an order in the order book
    fn update_order(
        &self,
        order_id: u64,
        new_price: Price,
        now_microseconds: u64,
    ) -> Result<(), UpdateOrderError> {
        let guard = &epoch::pin();
        let order_index = self.order_index.pin();
        let book_key = order_index.get(&order_id);
        let mut book_key = match book_key {
            Some(book_key) => *book_key,
            None => return Err(UpdateOrderError::OrderNotFound),
        };

        let order_entry_opt = match book_key.side {
            Side::Buy => self.buy_orders.get(&book_key, guard),
            Side::Sell => self.sell_orders.get(&book_key, guard),
        };
        let order_entry = match order_entry_opt {
            Some(order_entry) => order_entry,
            None => return Err(UpdateOrderError::OrderNotFound),
        };

        let book_order = order_entry.value();
        if !book_order.enter_finished_from_active() {
            return Err(UpdateOrderError::OrderNotModifiable);
        }

        let mut book_order = book_order.clone();
        order_index.remove(&order_id);
        order_entry.remove();

        // Set price、lifecycle before making visible in the book
        book_order.price = new_price;
        book_order.updated_at = now_microseconds;
        book_order.reset_lifecycle();
        book_key = book_order.book_key();

        // Insert into the book after lifecycle is set
        match book_order.side {
            Side::Buy => self.buy_orders.insert(book_key, book_order.clone(), guard),
            Side::Sell => self.sell_orders.insert(book_key, book_order.clone(), guard),
        };
        order_index.insert(book_order.id, book_key);
        let id = self.id.fetch_add(1, Ordering::Acquire);
        self.syncer.update_order(id, &book_order);

        Ok(())
    }

    /// remove an order from the order book
    fn remove(&self, order_id: u64) -> Result<(), CancelOrderError> {
        let guard = &epoch::pin();
        let order_index = self.order_index.pin();
        let book_key = order_index.get(&order_id);
        let book_key = match book_key {
            Some(book_key) => *book_key,
            None => return Err(CancelOrderError::OrderNotFound),
        };

        let order_entry_opt = match book_key.side {
            Side::Buy => self.buy_orders.get(&book_key, guard),
            Side::Sell => self.sell_orders.get(&book_key, guard),
        };
        let order_entry = match order_entry_opt {
            Some(order_entry) => order_entry,
            None => return Err(CancelOrderError::OrderNotFound),
        };

        let book_order = order_entry.value();
        if !book_order.enter_finished_from_active() {
            return Err(CancelOrderError::OrderNotCancellable);
        }

        order_entry.remove();
        order_index.remove(&order_id);
        let id = self.id.fetch_add(1, Ordering::Acquire);
        self.syncer.cancel_order(id, &book_order);

        Ok(())
    }

    /// Gets the best price for a side
    fn get_best_price(&self, side: Side) -> Option<Price> {
        let guard = &epoch::pin();
        let entry = match side {
            Side::Buy => self.buy_orders.front(guard),
            Side::Sell => self.sell_orders.front(guard),
        };
        match entry {
            Some(entry) => Some(entry.key().price),
            None => None,
        }
    }

    fn get_book(&self, side: Side) -> &SkipList<BookKey, Order> {
        match side {
            Side::Buy => &self.buy_orders,
            Side::Sell => &self.sell_orders,
        }
    }

    /// Sync orders that are matched and trades
    fn sync_matched(&self, updated: &Vec<Order>, trades: &Vec<Trade>) {
        let id = self.id.fetch_add(1, Ordering::Acquire);
        self.syncer.matched(id, updated, trades);
    }
}

impl MatchingEngineWalker for DefaultOrderBook {
    fn walking_market_book(&self, walk: &mut dyn FnMut(&Order) -> WalkingResult) {
        let guard = &epoch::pin();
        let mut entry = self.market_orders.front(guard);
        while let Some(e) = entry {
            let order = e.value();
            let result = walk(order);
            if result.remove {
                e.remove();
            } else if result.exit {
                break;
            }
            entry = e.next();
        }
    }

    fn walking_book_maker(
        &self,
        side: Side,
        slip_price_option: Option<Price>,
        walk: &mut dyn FnMut(&Order) -> WalkingResult,
    ) {
        let guard = &epoch::pin();
        let book = match side {
            Side::Buy => &self.buy_orders,
            Side::Sell => &self.sell_orders,
        };

        let mut entry = book.front(guard);
        while let Some(e) = entry {
            let key = e.key();
            let order = e.value();

            if order.liquidity_directive == LiquidityDirective::TakerOnly {
                entry = e.next();
                continue;
            }

            if let Some(slip_price) = slip_price_option {
                match side {
                    Side::Buy => {
                        if key.price < slip_price {
                            break;
                        }
                    }
                    Side::Sell => {
                        if key.price > slip_price {
                            break;
                        }
                    }
                }
            }

            let result = walk(order);
            if result.remove {
                e.remove();
            } else if result.exit {
                break;
            }

            entry = e.next();
        }
    }

    fn walking_cross_taker(&self, walk: &mut dyn FnMut(&Order) -> WalkingResult) {
        let guard = &epoch::pin();

        let (mut buy_entry_opt, mut sell_entry_opt) =
            (self.buy_orders.front(guard), self.sell_orders.front(guard));
        while buy_entry_opt.is_some() || sell_entry_opt.is_some() {
            match (buy_entry_opt.as_ref(), sell_entry_opt.as_ref()) {
                (Some(buy_entry), Some(sell_entry)) => {
                    let buy_key = buy_entry.key();
                    let sell_key = sell_entry.key();

                    if buy_key.price < sell_key.price {
                        break;
                    }

                    let buy_order = buy_entry.value();
                    let sell_order = sell_entry.value();
                    let (buy_maker_only, sell_maker_only) = (
                        buy_order.liquidity_directive == LiquidityDirective::MakerOnly,
                        sell_order.liquidity_directive == LiquidityDirective::MakerOnly,
                    );

                    if buy_maker_only && sell_maker_only {
                        buy_entry_opt = buy_entry.next();
                        sell_entry_opt = sell_entry.next();
                        continue;
                    }

                    let taker = if buy_maker_only && !sell_maker_only {
                        sell_order
                    } else if sell_maker_only && !buy_maker_only {
                        buy_order
                    } else if buy_key.priority < sell_key.priority {
                        buy_order
                    } else {
                        sell_order
                    };
                    let taker_is_buy = taker.side == Side::Buy;

                    let result = walk(taker);
                    if result.exit {
                        break;
                    }

                    if taker_is_buy {
                        if result.remove {
                            buy_entry.remove();
                        }
                        buy_entry_opt = buy_entry.next();
                    } else {
                        if result.remove {
                            sell_entry.remove();
                        }
                        sell_entry_opt = sell_entry.next();
                    }
                }

                (Some(buy_entry), None) => {
                    let buy_key = buy_entry.key();
                    let sell_key = match self.sell_orders.front(guard) {
                        Some(sell_entry) => sell_entry.key(),
                        None => break,
                    };
                    if buy_key.price < sell_key.price {
                        break;
                    }

                    let buy_order = buy_entry.value();
                    if buy_order.liquidity_directive == LiquidityDirective::MakerOnly {
                        buy_entry_opt = buy_entry.next();
                        continue;
                    }
                    let result = walk(buy_order);
                    if result.exit {
                        break;
                    }
                    if result.remove {
                        buy_entry.remove();
                    }
                    buy_entry_opt = buy_entry.next();
                }

                (None, Some(sell_entry)) => {
                    let buy_key = match self.buy_orders.front(guard) {
                        Some(buy_entry) => buy_entry.key(),
                        None => break,
                    };
                    let sell_key = sell_entry.key();
                    if buy_key.price < sell_key.price {
                        break;
                    }

                    let sell_order = sell_entry.value();
                    if sell_order.liquidity_directive == LiquidityDirective::MakerOnly {
                        sell_entry_opt = sell_entry.next();
                        continue;
                    }
                    let result = walk(sell_order);
                    if result.exit {
                        break;
                    }
                    if result.remove {
                        sell_entry.remove();
                    }
                    sell_entry_opt = sell_entry.next();
                }

                (None, None) => break, // unreachable theoretically
            }
        }
    }

    fn walking_by_order_id_list(
        &self,
        order_id_list: &[OrderID],
        walk: &mut dyn FnMut(&Order) -> WalkingResult,
    ) {
        let guard = &epoch::pin();
        let order_index = self.order_index.pin();

        for order_id in order_id_list {
            let book_key = order_index.get(order_id);
            let book_key = match book_key {
                Some(book_key) => *book_key,
                None => continue,
            };

            let order_entry_opt = match book_key.side {
                Side::Buy => self.buy_orders.get(&book_key, guard),
                Side::Sell => self.sell_orders.get(&book_key, guard),
            };
            let order_entry = match order_entry_opt {
                Some(order_entry) => order_entry,
                None => continue,
            };

            let order = order_entry.value();
            let result = walk(order);
            if result.remove {
                order_entry.remove();
            } else if result.exit {
                break;
            }
        }
    }
}

impl OrderBookWalker for DefaultOrderBook {}
