//! JsValue - An opaque reference to a JavaScript value
//!
//! This type represents a reference to a JavaScript value on the JS heap.
//! It is reference-counted and will notify JS when dropped.

use std::fmt;
use std::rc::Rc;

/// Reserved function ID for dropping heap refs on JS side.
/// This should be handled specially in the JS runtime.
pub const DROP_HEAP_REF_FN_ID: u32 = 0xFFFFFFFF;

/// Inner type for JsValue that holds the heap ID.
struct JsValueInner {
    id: u64,
}

impl Drop for JsValueInner {
    fn drop(&mut self) {
        // Queue a drop call to JS via the batch system
        crate::batch::queue_js_drop(self.id);
    }
}

/// An opaque reference to a JavaScript heap object.
///
/// This type is the wry-bindgen equivalent of wasm-bindgen's `JsValue`.
/// It represents any JavaScript value and is used as the base type for
/// all imported JS types.
///
/// JsValue is intentionally opaque - you cannot inspect or create values
/// directly. All values come from JavaScript via the IPC protocol.
#[derive(Clone)]
pub struct JsValue {
    inner: Rc<JsValueInner>,
}

impl JsValue {
    /// Create a new JsValue from a heap ID.
    ///
    /// This is called internally when decoding a value from JS.
    pub(crate) fn from_id(id: u64) -> Self {
        Self {
            inner: Rc::new(JsValueInner { id }),
        }
    }

    /// Get the heap ID for this value.
    ///
    /// This is used internally for encoding values to send to JS.
    pub(crate) fn id(&self) -> u64 {
        self.inner.id
    }
}

impl fmt::Debug for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsValue")
            .field("id", &self.inner.id)
            .finish()
    }
}

impl PartialEq for JsValue {
    fn eq(&self, other: &Self) -> bool {
        self.inner.id == other.inner.id
    }
}

impl Eq for JsValue {}

impl std::hash::Hash for JsValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.id.hash(state);
    }
}
