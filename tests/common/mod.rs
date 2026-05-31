//! Shared test utilities for integration tests.
//!
//! This module is NOT compiled as a standalone test — each `tests/*.rs` file
//! declares `mod common;` and Rust treats `tests/common/mod.rs` as a module
//! rather than a test binary.
//!
//! Contains: MockApi, MockProgress, factory functions, and temp-dir helpers.

pub mod helpers;
pub mod mocks;
