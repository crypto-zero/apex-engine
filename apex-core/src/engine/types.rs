use crypto_bigint::{Limb, NonZero, Reciprocal, U256, U512, Zero};
use mimalloc::MiMalloc;
use std::cell::UnsafeCell;
use std::ops::Mul;
use std::sync::atomic::{AtomicU8, Ordering};

/// Global allocator
/// Requires the `mimalloc` feature to be enabled in the `Cargo.toml` file.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// OrderID is the type used for order IDs.
pub type OrderID = u64;

/// Price is the type used for prices in the order.
/// This is a 256-bit unsigned integer.
pub type Price = U256;

/// Quantity is the type used for quantities in the order.
/// This is a 256-bit unsigned integer.
pub type Quantity = U256;

/// Priority is that the order book uses to determine the order priority.
pub type Priority = u64;

/// Side indicates the direction of the order.
#[derive(PartialEq, Eq, Default, Copy, Clone, Debug)]
pub enum Side {
    /// Buy means the user wants to acquire the asset, typically matching against sell orders.
    #[default]
    Buy,
    /// Sell means the user wants to sell the asset, typically matching against buy orders.
    Sell,
}

/// OrderType determines how the order will be executed.
#[derive(PartialEq, Eq, Default, Copy, Clone, Debug)]
pub enum OrderType {
    /// Limit orders specify a maximum (for buy) or minimum (for sell) price and can be added to the book.
    #[default]
    Limit,
    /// Market orders do not specify a price and must be filled immediately against the best available prices.
    Market,
}

/// OrderStatus represents the current status of an order during its lifecycle.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum OrderStatus {
    /// The order has been received and is waiting to be processed.
    #[default]
    Pending,
    /// The order is currently active and can be matched against other orders.
    Placed,
    /// The order was fully filled.
    Filled,
    /// The order was partially filled but still has the remaining quantity.
    PartiallyFilled,
    /// The order was canceled before being fully filled.
    Cancelled,
    /// The order was rejected (e.g., invalid, unauthorized).
    Rejected,
    /// The order expired due to its time-in-force condition (e.g., GoodTillDate).
    Expired,
}

/// Represents the lifecycle state of an order.
/// This enum is used to coordinate safe concurrent access between
/// matching threads and cancellation threads.
///
/// Matching and cancellation threads use atomic state transitions
/// to claim or skip processing orders.
///
/// The transitions are:
/// - `Active` → `Matched` (matching thread claims order)
/// - `Active` → `Finished` (cancellation thread removes order)
/// - `Matched` → `Active` (matching thread partially fills order)
/// - `Matched` → `Finished` (matching thread completes order)
/// So finally state is `Finished`.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum OrderLifecycle {
    /// The order is live and can be matched or canceled.
    #[default]
    Active = 0,

    /// The order is currently being matched and cannot be canceled.
    Matched = 1,

    /// The order has been finished matching and can be removed from the order book.
    Finished = 2,
}

/// CancelReason indicates the reason for canceling an order.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum CancelReason {
    /// The user canceled the order.
    #[default]
    UserRequest,
    /// The order was canceled due to a timeout or expiration.
    TimeInForceExpired,
}

/// RejectReason indicates the reason for rejecting an order.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum RejectReason {
    /// The order was rejected due to timestamp conflicts.
    #[default]
    TimestampConflict,
    /// The order was rejected due to insufficient liquidity.
    /// This can happen if the order is a market order and there are not enough matching orders.
    InsufficientLiquidity,
}

/// MatchStrategy represents the strategy used to match an order.
/// It defines how aggressively or restrictively an order should be matched.
#[derive(PartialEq, Eq, Default, Copy, Clone, Debug)]
pub enum MatchStrategy {
    /// Standard matching allows partial fills and placing the remainder on the book.
    #[default]
    Standard,
    /// FillOrKill requires the full quantity to be matched immediately or the order is canceled.
    FillOrKill,
    /// ImmediateOrCancel allows partial immediate fills; any remainder is canceled.
    ImmediateOrCancel,
}

/// LiquidityDirective specifies whether the order is allowed to take or must provide liquidity.
/// It determines whether an order can match against existing orders
/// (taker) or only rest in the book (maker).
#[derive(PartialEq, Eq, Default, Copy, Clone, Debug)]
pub enum LiquidityDirective {
    /// AllowTaker means the order is allowed to match against existing orders.
    #[default]
    AllowTaker,
    /// MakerOnly means the order must only add liquidity; if it matches, it is canceled.
    MakerOnly,
    /// TakerOnly means the order must immediately match against resting orders;
    /// it will be rejected if it rests on the book.
    TakerOnly,
}

/// TimeInForce specifies how long the order remains active on the order book.
#[derive(PartialEq, Eq, Default, Copy, Clone, Debug)]
pub enum TimeInForce {
    /// None means
    /// the order will be executed immediately and not placed in the book.
    #[default]
    None,
    /// GoodTillCancelled means
    /// the order will remain active until it is either filled or manually canceled.
    GoodTillCancelled,
    /// GoodTillDate keeps the order valid until a specified timestamp.
    GoodTillDate(u64),
}

/// SlippageTolerance defines the maximum acceptable price deviation for an order,
/// expressed in basis points (bps), where 1% = 100 bps.
///
/// For example,
/// - A value of `50` means the user accepts up to 0.50% slippage.
/// - A value of `0` indicates strict price protection (no slippage allowed).
///
/// Slippage is typically used with market or taker-style orders, where the
/// final execution price might differ from the quoted price due to market movement.
///
/// This field should be `None` for limit orders that already specify an explicit price.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct SlippageTolerance(pub u32);

/// Maximum slippage tolerance allowed.
/// This is set to 50% (5000 bps).
pub const MAX_ALLOWED_SLIPPAGE_TOLERANCE: SlippageTolerance = SlippageTolerance(5000);

/// a constant used for calculating slippage tolerance.
const RECIPROCAL_10000: Reciprocal = Reciprocal::new(NonZero::<Limb>::new_unwrap(Limb(10_000u64)));

/// BookKey is a composite key for identifying an order's position in the book.
/// It combines the order's price, priority (timestamp-based), and side (Buy/Sell).
///
/// The ordering semantics are:
/// - For Buy orders: higher prices are prioritized (sorted descending),
///   and for the same price, earlier orders (lower priority values) are prioritized.
/// - For Sell orders: lower prices are prioritized (sorted ascending),
///   and for the same price, earlier orders (lower priority values) are prioritized.
///
/// This allows a single skip list to sort all orders per side correctly,
/// without needing a secondary level of price grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookKey {
    pub price: Price,
    pub priority: Priority,
    pub side: Side,
}

impl Ord for BookKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.side {
            Side::Buy => {
                // Higher price first for buys, then earlier priority
                self.price
                    .cmp(&other.price)
                    .reverse()
                    .then(self.priority.cmp(&other.priority))
            }
            Side::Sell => {
                // Lower price first for sells, then earlier priority
                self.price
                    .cmp(&other.price)
                    .then(self.priority.cmp(&other.priority))
            }
        }
    }
}

impl PartialOrd for BookKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `Order` represents a single order in the book.
///
/// Certain fields (quantity, filled_quantity, status, cancel_reason, reject_reason)
/// are wrapped with `UnsafeCell` to allow safe internal mutability.
///
/// SAFETY: All unsafe mutations are controlled within the matching engine thread
/// context to prevent data races and maintain logical soundness.
#[derive(Debug)]
pub struct Order {
    pub id: OrderID,
    pub user_id: u64,
    pub side: Side,
    pub lifecycle: AtomicU8,
    pub order_type: OrderType,
    pub status: UnsafeCell<OrderStatus>,
    pub match_strategy: MatchStrategy,
    pub liquidity_directive: LiquidityDirective,
    pub time_in_force: TimeInForce,
    pub price: Price,
    pub slippage_tolerance: Option<SlippageTolerance>,
    pub quantity: UnsafeCell<Quantity>,
    // TODO: iceberg orders design
    // pub visible_quantity: Option<Quantity>, // if None, fully visible
    pub filled_quantity: UnsafeCell<Quantity>,
    pub cancel_reason: UnsafeCell<Option<CancelReason>>,
    pub reject_reason: UnsafeCell<Option<RejectReason>>,
    pub created_at: u64, // In microseconds
    pub updated_at: u64, // In microseconds
}

/// OrderValidationError represents possible validation failures for order parameters.
#[derive(Debug)]
pub enum OrderValidationError {
    /// The match strategy used is invalid for an order.
    InvalidMatchStrategy,
    /// The time-in-force value is invalid for an order.
    InvalidTimeInForce,
    /// The liquidity directive is invalid for an order.
    InvalidLiquidityDirective,
    /// The slippage tolerance is not applicable for the order type.
    SlippageNotApplicable,
    /// The slippage tolerance exceeds the maximum allowed value.
    SlippageExceedsMaximum,
}

/// TradeRole represents the role of the order in a matched trade.
/// Maker is the resting order already in the book;
/// Taker is the incoming order that triggers the match.
#[derive(PartialEq, Eq, Default, Clone, Copy, Debug)]
pub enum TradeRole {
    /// Maker indicates the order was already resting in the order book and provided liquidity.
    #[default]
    Maker = 0,
    /// Taker indicates the order was newly submitted and removed liquidity from the order book.
    Taker = 1,
}

/// Trade represents a trade matched in the orders.
#[derive(Default, Clone, Debug)]
pub struct Trade {
    pub role: TradeRole,
    pub order_id: u64,
    pub price: Price,
    pub quantity: Quantity,
    pub created_at: u64,
}

impl From<u8> for OrderLifecycle {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Active,
            1 => Self::Matched,
            2 => Self::Finished,
            _ => unreachable!("Invalid lifecycle state"),
        }
    }
}

impl From<OrderLifecycle> for u8 {
    fn from(l: OrderLifecycle) -> u8 {
        l as u8
    }
}

impl Default for Order {
    fn default() -> Self {
        Order {
            id: 0,
            user_id: 0,
            side: Side::default(),
            lifecycle: AtomicU8::new(OrderLifecycle::Active.into()),
            order_type: OrderType::default(),
            status: UnsafeCell::new(OrderStatus::default()),
            match_strategy: MatchStrategy::default(),
            liquidity_directive: LiquidityDirective::default(),
            time_in_force: TimeInForce::default(),
            price: U256::ZERO,
            slippage_tolerance: None,
            quantity: UnsafeCell::new(U256::ZERO),
            filled_quantity: UnsafeCell::new(U256::ZERO),
            cancel_reason: UnsafeCell::new(None),
            reject_reason: UnsafeCell::new(None),
            created_at: 0,
            updated_at: 0,
        }
    }
}

impl Clone for Order {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            user_id: self.user_id,
            side: self.side,
            lifecycle: AtomicU8::new(self.lifecycle.load(Ordering::Acquire).into()),
            order_type: self.order_type,
            status: UnsafeCell::new(unsafe { *self.status.get() }),
            match_strategy: self.match_strategy,
            liquidity_directive: self.liquidity_directive,
            time_in_force: self.time_in_force,
            price: self.price,
            slippage_tolerance: self.slippage_tolerance,
            quantity: UnsafeCell::new(unsafe { *self.quantity.get() }),
            filled_quantity: UnsafeCell::new(unsafe { *self.filled_quantity.get() }),
            cancel_reason: UnsafeCell::new(unsafe { (*self.cancel_reason.get()).clone() }),
            reject_reason: UnsafeCell::new(unsafe { (*self.reject_reason.get()).clone() }),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

unsafe impl Sync for Order {}

impl Order {
    /// Get the order's status.
    #[inline(always)]
    pub fn status(&self) -> OrderStatus {
        unsafe { *self.status.get() }
    }

    /// Check the order status is filled.
    #[inline(always)]
    pub fn is_filled(&self) -> bool {
        self.status() == OrderStatus::Filled
    }

    /// Get the quantity of the order.
    #[inline(always)]
    pub fn quantity(&self) -> Quantity {
        unsafe { *self.quantity.get() }
    }

    /// Get the filled quantity of the order.
    #[inline(always)]
    pub fn filled_quantity(&self) -> Quantity {
        unsafe { *self.filled_quantity.get() }
    }

    /// Get the book key for the order.
    #[inline(always)]
    pub fn book_key(&self) -> BookKey {
        BookKey {
            price: self.price,
            priority: self.priority(),
            side: self.side,
        }
    }

    /// Get the current lifecycle state is `Finished`.
    #[inline(always)]
    pub(crate) fn is_finished(&self) -> bool {
        self.lifecycle.load(Ordering::Acquire) == OrderLifecycle::Finished.into()
    }

    /// Reset lifecycle state to `Active`.
    #[inline(always)]
    pub(crate) fn reset_lifecycle(&self) {
        self.lifecycle
            .store(OrderLifecycle::Active.into(), Ordering::Release);
    }

    /// Enter matched lifecycle state.
    #[inline(always)]
    pub(crate) fn enter_matched(&self) -> bool {
        match self.lifecycle.compare_exchange_weak(
            OrderLifecycle::Active.into(),
            OrderLifecycle::Matched.into(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Exit from matched to active lifecycle state.
    #[inline(always)]
    pub(crate) fn exit_matched(&self) -> bool {
        match self.lifecycle.compare_exchange_weak(
            OrderLifecycle::Matched.into(),
            OrderLifecycle::Active.into(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Enter the finished lifecycle state from active.
    #[inline(always)]
    pub(crate) fn enter_finished_from_active(&self) -> bool {
        match self.lifecycle.compare_exchange_weak(
            OrderLifecycle::Active.into(),
            OrderLifecycle::Finished.into(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Enter the finished lifecycle state from matched.
    #[inline(always)]
    pub(crate) fn enter_finished_from_matched(&self) -> bool {
        match self.lifecycle.compare_exchange_weak(
            OrderLifecycle::Matched.into(),
            OrderLifecycle::Finished.into(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Get the order priority of the order book.
    #[inline(always)]
    pub(crate) fn priority(&self) -> Priority {
        // The order's priority is determined by its creation timestamp.
        // The Earlier the order, the higher the priority.
        self.updated_at * 100 + self.id % 100
    }

    /// SAFETY:
    /// Only the matching engine thread modifies quantity and filled_quantity,
    /// ensuring no data race even though accessed through shared reference.
    #[inline(always)]
    pub(crate) fn quantity_fill(&self, traded: Quantity) -> Quantity {
        unsafe {
            *self.quantity.get() -= traded;
            *self.filled_quantity.get() += traded;
            *self.quantity.get()
        }
    }

    /// SAFETY:
    /// Only the matching engine thread modifies order status through shared reference,
    /// ensuring no concurrent modification.
    #[inline(always)]
    pub(crate) fn update_status(&self, status: OrderStatus) {
        unsafe {
            *self.status.get() = status;
        }
    }

    /// SAFETY:
    /// Only the matching engine thread modifies cancel_reason,
    /// ensuring safe access under shared reference.
    #[inline(always)]
    pub(crate) fn update_cancel_reason(&self, reason: CancelReason) {
        unsafe {
            *self.cancel_reason.get() = Some(reason);
        }
    }

    /// SAFETY:
    /// Only the matching engine thread modifies reject_reason,
    /// ensuring safe access under shared reference.
    #[inline(always)]
    pub(crate) fn update_reject_reason(&self, reason: RejectReason) {
        unsafe {
            *self.reject_reason.get() = Some(reason);
        }
    }

    /// Returns the worst acceptable execution price under slippage tolerance.
    ///
    /// This function computes a boundary price based on the best price and slippage tolerance (in bps).
    /// For 'Buy' orders, it returns the highest price the user is willing to accept;
    /// For 'Sell' orders, the lowest price.
    /// Returns `None` if no slippage tolerance is set.
    pub fn slippage_bound_price(&self, price: Price) -> Option<Price> {
        if self.slippage_tolerance.is_none() {
            return None;
        }

        let slippage = self.slippage_tolerance.unwrap();
        let mut factor = U512::from(slippage.0);
        factor = factor.mul(price);
        let (quotient, _) = factor.div_rem_limb_with_reciprocal(&RECIPROCAL_10000);
        let (lo, _) = quotient.split();
        let bound_price = match self.side {
            Side::Buy => price + lo,
            Side::Sell => price - lo,
        };
        Some(bound_price)
    }

    /// Validates the order for correctness.
    pub fn validate(&self) -> Result<(), OrderValidationError> {
        match self.order_type {
            OrderType::Limit => {
                // 1. MatchStrategy must be Standard
                match self.match_strategy {
                    MatchStrategy::Standard => {}
                    _ => return Err(OrderValidationError::InvalidMatchStrategy),
                }
                // 2. LiquidityDirective must be AllowTaker or MakerOnly
                match self.liquidity_directive {
                    LiquidityDirective::AllowTaker | LiquidityDirective::MakerOnly => {}
                    _ => return Err(OrderValidationError::InvalidLiquidityDirective),
                }
                // 3. TimeInForce must be GoodTillCancelled or GoodTillDate
                match self.time_in_force {
                    TimeInForce::GoodTillCancelled | TimeInForce::GoodTillDate(_) => {}
                    _ => return Err(OrderValidationError::InvalidTimeInForce),
                }
                // 4. SlippageTolerance must be None
                if self.slippage_tolerance.is_some() {
                    return Err(OrderValidationError::SlippageNotApplicable);
                }

                Ok(())
            }
            OrderType::Market => {
                // 1. MatchStrategy must be IOC or FOK
                match self.match_strategy {
                    MatchStrategy::ImmediateOrCancel | MatchStrategy::FillOrKill => {}
                    _ => return Err(OrderValidationError::InvalidMatchStrategy),
                }
                // 2. LiquidityDirective must not be MakerOnly
                if self.liquidity_directive == LiquidityDirective::MakerOnly {
                    return Err(OrderValidationError::InvalidLiquidityDirective);
                }
                // 3. TimeInForce must NOT be GoodTillCancelled or GoodTillDate
                match self.time_in_force {
                    TimeInForce::GoodTillCancelled | TimeInForce::GoodTillDate(_) => {
                        return Err(OrderValidationError::InvalidTimeInForce);
                    }
                    _ => {}
                }
                // 4. SlippageTolerance could be None or a valid value
                if let Some(slippage) = self.slippage_tolerance {
                    if slippage.0 > MAX_ALLOWED_SLIPPAGE_TOLERANCE.0 {
                        return Err(OrderValidationError::SlippageExceedsMaximum);
                    }
                }

                Ok(())
            }
        }
    }

    /// Clone the order and reset its lifecycle state to `Active`.
    pub(crate) fn clone_reset_lifecycle(&self) -> Self {
        let cloned = self.clone();
        cloned.reset_lifecycle();
        cloned
    }
}

impl Trade {
    /// Orders matched then calculate the quantity and trades.
    #[inline(always)]
    pub(crate) fn matched(
        now_microseconds: u64,
        taker: &Order,
        maker: &Order,
    ) -> Option<(Trade, Trade)> {
        let mut maker_quantity = maker.quantity();
        let mut taker_quantity = taker.quantity();
        let traded_quantity = taker_quantity.min(maker_quantity);
        if traded_quantity.is_zero().into() {
            return None;
        }

        maker_quantity = maker.quantity_fill(traded_quantity);
        taker_quantity = taker.quantity_fill(traded_quantity);

        let maker_status = if maker_quantity.is_zero().into() {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };
        let taker_status = if taker_quantity.is_zero().into() {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };

        maker.update_status(maker_status);
        taker.update_status(taker_status);

        Some((
            Trade {
                role: TradeRole::Maker,
                order_id: maker.id,
                price: maker.price,
                quantity: traded_quantity,
                created_at: now_microseconds,
            },
            Trade {
                role: TradeRole::Taker,
                order_id: taker.id,
                price: maker.price,
                quantity: traded_quantity,
                created_at: now_microseconds,
            },
        ))
    }
}
