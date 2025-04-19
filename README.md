# ⚡ ApexEngine - High-Performance Matching Engine

> **ApexEngine** is a next-generation, high-performance, lock-free matching engine, designed for ultra-low latency trading systems and scalable exchange infrastructures.  
> **Reach the Apex of Matching Performance.**

> **Requires Rust 1.88 nightly.**

[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue)](#-license)
[![Rust Version](https://img.shields.io/badge/rust-1.88%20nightly-orange)](https://www.rust-lang.org/)

---

## ✨ Features

- **Lock-free Order Book**
- **Limit and Market Orders**
- **Thread-safe Multi-Core Scalability**
- **Priority-based Matching**
- **Slippage Tolerance**
- **Comprehensive Matching Strategies**
- **High Performance Benchmarks**
- **Memory Efficient**

---

## 📖 Design Overview

- Skiplist-based concurrent order book
- Safe atomic state transitions for lifecycle
- Market orders with optional slippage tolerance

---

## 🚀 Benchmarking

Benchmark ApexEngine's matching performance with the following scenarios:

```bash
cargo bench
```

| Scenario                               | Description                          |
|:---------------------------------------|:-------------------------------------|
| `match_orders 10k`                     | Insert and match 10,000 limit orders |
| `multi-thread insert/cancel/match TPS` | Concurrent insert/cancel/match       |

---

## 🛠 Usage Example

```rust
use matching_engine::prelude::*;
use matching_engine::DefaultMatchingEngine;
use std::sync::Arc;
use std::sync::atomic::AtomicU128;

let syncer = Arc::new(EmptyOrderBookSyncer {});
let id = Arc::new(AtomicU128::new(1));
let book = Arc::new(DefaultOrderBook::new(id, syncer));
let engine = DefaultMatchingEngine::new(book);

let mut order = make_limit_order(1, Side::Buy, 1000, 10, 1000000);
engine.create_order(&mut order).unwrap();

engine.match_orders();
```

---

## 📦 Dependencies

- [crossbeam](https://docs.rs/crossbeam/)
- [crossbeam-skiplist](https://docs.rs/crossbeam-skiplist/)
- [criterion](https://docs.rs/criterion/)
- [mimalloc](https://docs.rs/mimalloc/)
- [rand](https://docs.rs/rand/)

---

## 📈 Roadmap

- [ ] Iceberg Orders
- [ ] Advanced Order Types
- [ ] Multi-Symbol Support
- [ ] Persistent Storage

---

# 🛎️ Why ApexEngine?

Because every microsecond matters.  
ApexEngine is engineered for ultra-high performance, deterministic matching, and scalable concurrency — **without sacrificing safety**.

---

## 🤝 Contributing

Contributions, issues and pull requests are welcome!

---

# 📜 License

This project is dual-licensed under:

- [MIT license](./LICENSE-MIT)
- [Apache License 2.0](./LICENSE-APACHE)

at your option.

You may choose either license when using this software.