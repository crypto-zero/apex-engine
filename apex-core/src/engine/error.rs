/// Represents possible errors when trying to update an order.
#[derive(Debug)]
pub enum UpdateOrderError {
    /// The order was not found in the book.
    OrderNotFound,
    /// The order is not in a modifiable state (e.g., already matched or canceled).
    OrderNotModifiable,
    /// The requested update is invalid (e.g., price change not allowed).
    InvalidUpdateRequest,
}

/// Represents possible errors when trying to cancel an order.
#[derive(Debug)]
pub enum CancelOrderError {
    /// The order was not found in the book.
    OrderNotFound,
    /// The order is not in a cancellable state (e.g., already matched).
    OrderNotCancellable,
    /// The requested cancel is invalid (e.g., order already canceled).
    InvalidCancelRequest,
}
