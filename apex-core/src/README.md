# Matching Engine Test Plan

## Functional Matrix

| Feature Area                                          | Supported | Test Coverage   |
|:------------------------------------------------------|:----------|:----------------|
| Limit Order Insertion                                 | ✅         | ✅ Full coverage |
| Limit Order Matching (Partial / Full Fill)            | ✅         | ✅ Full coverage |
| Market Order Matching (IOC / FOK)                     | ✅         | ✅ Full coverage |
| Slippage Protection (Market Orders)                   | ✅         | ✅ Full coverage |
| MakerOnly / TakerOnly Constraints                     | ✅         | ✅ Full coverage |
| Order Cancellation                                    | ✅         | ✅ Full coverage |
| Order Price Update                                    | ✅         | ✅ Full coverage |
| Order Lifecycle Transitions (Active/Matched/Finished) | ✅         | ✅ Full coverage |
| Handling Matching Failures (Insufficient Liquidity)   | ✅         | ❌ To be added   |
| Error Code Propagation (RejectReason, CancelReason)   | ✅         | ❌ To be added   |

## Test Coverage Roadmap

We propose expanding unit tests in the following recommended sequence:

### 1. Limit Order Matching

- Partial fill scenarios
- Full fill scenarios
- Verify remaining quantity and order book state
- Ensure correct lifecycle state transitions (`Placed → Matched → Filled/PartiallyFilled`)

### 2. Market Order Matching

- ImmediateOrCancel (IOC) behavior
- FillOrKill (FOK) behavior
- Handling of insufficient liquidity rejection
- Slippage tolerance application during matching

### 3. Slippage Protection Tests

- Market order exceeds slippage bound and gets rejected
- Market order within slippage bound gets filled normally

### 4. MakerOnly / TakerOnly Constraints

- MakerOnly orders must not immediately match; if matching opportunity exists, matching is skipped
- TakerOnly orders must immediately match; if no immediate matching opportunity, order remains unmatched
- No cancel or reject is triggered solely by MakerOnly or TakerOnly directive
- Ensure correct handling of liquidity directives without affecting order book integrity

### 5. Order Cancellation Tests

- Cancel an active order
- Cancel a partially filled order
- Verify correct cancellation reason
- Ensure order is removed from the book

### 6. Order Price Update Tests

- Update an active order’s price
- Verify correct reordering in the book
- Ensure no race conditions on updated orders

### 7. Matching Failure Handling

- Attempt to match when no opposite side liquidity
- Properly reject market orders with `InsufficientLiquidity`
- Ensure correct order lifecycle and rejection reason

### 8. Error Code Validation

- Confirm that every failure (cancel, update, insert) returns correct structured error
- Ensure client-side can correctly interpret failure cases

## Future Testing Enhancements

In the future, we plan to introduce the following improvements:

- **Stress Testing:**
    - High volume insertion, cancellation, and matching
    - Validate memory safety and absence of panics
- **Concurrent Matching Simulation:**
    - Parallel matching + cancellation threads
    - Validate lifecycle state race protections
- **Benchmark Testing:**
    - Measure matching latency, order insertion latency, cancellation latency
- **Advanced Matching Features:**
    - Mid-price matching scenarios
    - Iceberg orders (partial visible quantity)

## Summary

This document outlines the current matching engine test plan, including the current state of coverage and the roadmap
for full feature validation.  
Following this plan will ensure correctness, stability, and maintainability of the matching engine over time.