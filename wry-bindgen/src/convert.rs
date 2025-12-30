//! Conversion traits for wasm-bindgen API compatibility.
//!
//! These traits provide compatibility with code that uses wasm-bindgen's
//! low-level ABI conversion types.

use crate::JsValue;

/// Trait for converting a Rust type to its WebAssembly ABI representation.
///
/// In wry-bindgen, this is simplified since we don't use the WebAssembly ABI directly.
/// Instead, we use heap indices (u64) for JsValue references.
pub trait IntoWasmAbi {
    /// The ABI type that this converts into.
    type Abi;

    /// Convert this value into its ABI representation.
    fn into_abi(self) -> Self::Abi;
}

/// Trait for converting from a WebAssembly ABI representation to a Rust type.
///
/// # Safety
/// The caller must ensure the ABI value is valid for the target type.
pub trait FromWasmAbi {
    /// The ABI type that this converts from.
    type Abi;

    /// Convert from the ABI representation to this type.
    ///
    /// # Safety
    /// The caller must ensure the ABI value is valid.
    unsafe fn from_abi(js: Self::Abi) -> Self;
}

/// Trait for creating a reference from a WebAssembly ABI representation.
///
/// This is used when you need to pass a reference to JS without transferring ownership.
pub trait RefFromWasmAbi {
    /// The ABI type that this converts from.
    type Abi;

    /// The anchor type that keeps the reference valid.
    type Anchor: core::ops::Deref<Target = Self>;

    /// Create a reference from the ABI representation.
    ///
    /// # Safety
    /// The caller must ensure the ABI value is valid.
    unsafe fn ref_from_abi(js: Self::Abi) -> Self::Anchor;
}

/// Trait for types that can provide a mutable reference from a WebAssembly ABI representation.
pub trait RefMutFromWasmAbi {
    /// The ABI type that this converts from.
    type Abi;

    /// The mutable anchor type.
    type Anchor: core::ops::DerefMut<Target = Self>;

    /// Create a mutable reference from the ABI representation.
    ///
    /// # Safety
    /// The caller must ensure the ABI value is valid.
    unsafe fn ref_mut_from_abi(js: Self::Abi) -> Self::Anchor;
}

// JsValue uses u32 as its ABI type for wasm-bindgen compatibility
// (internally we use u64, but the ABI layer uses u32 for compatibility)
impl IntoWasmAbi for JsValue {
    type Abi = u32;

    fn into_abi(self) -> Self::Abi {
        let id = self.id();
        core::mem::forget(self); // Don't drop - ownership transferred
        id as u32
    }
}

impl FromWasmAbi for JsValue {
    type Abi = u32;

    unsafe fn from_abi(js: Self::Abi) -> Self {
        JsValue::from_id(js as u64)
    }
}

impl RefFromWasmAbi for JsValue {
    type Abi = u32;
    type Anchor = JsValueRef;

    unsafe fn ref_from_abi(js: Self::Abi) -> Self::Anchor {
        JsValueRef(JsValue::from_id(js as u64))
    }
}

/// A reference wrapper for JsValue that implements Deref.
///
/// This is used as the anchor type for `RefFromWasmAbi`.
pub struct JsValueRef(pub(crate) JsValue);

impl core::ops::Deref for JsValueRef {
    type Target = JsValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Implement for reference types
impl IntoWasmAbi for &JsValue {
    type Abi = u32;

    fn into_abi(self) -> Self::Abi {
        self.id() as u32
    }
}

// Implement for Option<JsValue>
impl IntoWasmAbi for Option<JsValue> {
    type Abi = u32;

    fn into_abi(self) -> Self::Abi {
        match self {
            Some(val) => val.into_abi(),
            None => 0, // Use 0 as sentinel for None
        }
    }
}
