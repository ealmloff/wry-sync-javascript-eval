//! Unified wasm-bindgen shim crate
//!
//! This crate transparently re-exports either:
//! - wry-bindgen-core for desktop targets (non-wasm32)
//! - wasm-bindgen for wasm32 targets

#![no_std]
#![allow(hidden_glob_reexports)]

#[cfg(not(target_arch = "wasm32"))]
pub use wry_bindgen::*;

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_upstream::*;
