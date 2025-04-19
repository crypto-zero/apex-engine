#![feature(integer_atomics)]

pub mod engine;

pub mod prelude {
    pub use crate::engine::prelude::*;
}
