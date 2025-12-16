//! JsCast - Type casting trait for JavaScript types
//!
//! This trait provides methods for casting between JavaScript types,
//! similar to wasm-bindgen's JsCast trait.

use crate::JsValue;

/// Trait for types that can be cast to and from JsValue.
///
/// This is the wry-bindgen equivalent of wasm-bindgen's `JsCast` trait.
/// It enables safe and unsafe casting between JavaScript types.
pub trait JsCast: AsRef<JsValue> + Into<JsValue> + Sized {
    /// Check if a JsValue is an instance of this type.
    ///
    /// This performs a runtime instanceof check in JavaScript.
    fn instanceof(val: &JsValue) -> bool;

    /// Unchecked cast from JsValue to this type.
    ///
    /// # Safety
    /// This does not perform any runtime checks. The caller must ensure
    /// the value is actually of the correct type.
    fn unchecked_from_js(val: JsValue) -> Self;

    /// Unchecked cast from a JsValue reference to a reference of this type.
    ///
    /// # Safety
    /// This does not perform any runtime checks. The caller must ensure
    /// the value is actually of the correct type.
    fn unchecked_from_js_ref(val: &JsValue) -> &Self;

    /// Try to cast a JsValue to this type.
    ///
    /// Returns `Ok(Self)` if the value is an instance of this type,
    /// otherwise returns `Err(val)` with the original value.
    fn dyn_into(val: JsValue) -> Result<Self, JsValue> {
        if Self::instanceof(&val) {
            Ok(Self::unchecked_from_js(val))
        } else {
            Err(val)
        }
    }

    /// Try to get a reference to this type from a JsValue reference.
    ///
    /// Returns `Some(&Self)` if the value is an instance of this type,
    /// otherwise returns `None`.
    fn dyn_ref(val: &JsValue) -> Option<&Self> {
        if Self::instanceof(val) {
            Some(Self::unchecked_from_js_ref(val))
        } else {
            None
        }
    }

    /// Check if this value is an instance of another type.
    fn is_instance_of<T: JsCast>(&self) -> bool {
        T::instanceof(self.as_ref())
    }

    /// Unchecked cast to another type.
    fn unchecked_into<T: JsCast>(self) -> T {
        T::unchecked_from_js(self.into())
    }

    /// Unchecked cast to a reference of another type.
    fn unchecked_ref<T: JsCast>(&self) -> &T {
        T::unchecked_from_js_ref(self.as_ref())
    }
}

/// Implement JsCast for JsValue itself (identity cast)
impl JsCast for JsValue {
    fn instanceof(_val: &JsValue) -> bool {
        true // Everything is a JsValue
    }

    fn unchecked_from_js(val: JsValue) -> Self {
        val
    }

    fn unchecked_from_js_ref(val: &JsValue) -> &Self {
        val
    }
}

impl AsRef<JsValue> for JsValue {
    fn as_ref(&self) -> &JsValue {
        self
    }
}
