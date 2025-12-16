//! JavaScript function references and Rust callback management.
//!
//! This module provides types for calling JavaScript functions from Rust
//! and for registering Rust callbacks that can be called from JavaScript.

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;

use slotmap::{DefaultKey, Key, SlotMap};

use crate::batch::run_js_sync;
use crate::encode::{BatchableResult, BinaryEncode, RustCallbackMarker, TypeConstructor};
use crate::ipc::{DecodedData, EncodedData};

/// Reserved function ID for dropping native Rust refs when JS objects are GC'd.
/// JS sends this when a FinalizationRegistry callback fires for a RustFunction.
pub const DROP_NATIVE_REF_FN_ID: u32 = 0xFFFFFFFF;

/// A reference to a JavaScript function that can be called from Rust.
///
/// The type parameter encodes the function signature.
/// Arguments and return values are serialized using the binary protocol.
pub struct JSFunction<T> {
    id: u32,
    function: PhantomData<T>,
}

impl<T> JSFunction<T> {
    pub const fn new(id: u32) -> Self {
        Self {
            id,
            function: PhantomData,
        }
    }

    /// Get the function ID.
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl<R: BatchableResult> JSFunction<fn() -> R> {
    pub fn call(&self) -> R {
        run_js_sync::<R>(self.id, |_| {})
    }
}

impl<T1, R: BatchableResult> JSFunction<fn(T1) -> R> {
    pub fn call<P1>(&self, arg: T1) -> R
    where
        T1: BinaryEncode<P1>,
    {
        run_js_sync::<R>(self.id, |encoder| {
            arg.encode(encoder);
        })
    }
}

impl<T1, T2, R: BatchableResult> JSFunction<fn(T1, T2) -> R> {
    pub fn call<P1, P2>(&self, arg1: T1, arg2: T2) -> R
    where
        T1: BinaryEncode<P1>,
        T2: BinaryEncode<P2>,
    {
        run_js_sync::<R>(self.id, |encoder| {
            arg1.encode(encoder);
            arg2.encode(encoder);
        })
    }
}

impl<T1, T2, T3, R: BatchableResult> JSFunction<fn(T1, T2, T3) -> R> {
    pub fn call<P1, P2, P3>(&self, arg1: T1, arg2: T2, arg3: T3) -> R
    where
        T1: BinaryEncode<P1>,
        T2: BinaryEncode<P2>,
        T3: BinaryEncode<P3>,
    {
        run_js_sync::<R>(self.id, |encoder| {
            arg1.encode(encoder);
            arg2.encode(encoder);
            arg3.encode(encoder);
        })
    }
}

/// Internal type for storing Rust callback functions.
pub(crate) struct RustValue {
    pub(crate) f: Box<dyn FnMut(&mut DecodedData, &mut EncodedData)>,
}

impl RustValue {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(&mut DecodedData, &mut EncodedData) + 'static,
    {
        Self { f: Box::new(f) }
    }
}

/// Encoder for storing Rust objects that can be called from JS.
pub(crate) struct ObjEncoder {
    pub(crate) functions: SlotMap<DefaultKey, Option<Box<dyn Any>>>,
}

impl ObjEncoder {
    pub(crate) fn new() -> Self {
        Self {
            functions: SlotMap::new(),
        }
    }

    pub(crate) fn register_value<T: 'static>(&mut self, value: T) -> DefaultKey {
        self.functions.insert(Some(Box::new(value)))
    }
}

thread_local! {
    pub(crate) static THREAD_LOCAL_FUNCTION_ENCODER: RefCell<ObjEncoder> = RefCell::new(ObjEncoder::new());
}

/// Register a callback with the thread-local encoder using a short borrow
pub(crate) fn register_callback(callback: RustValue) -> DefaultKey {
    THREAD_LOCAL_FUNCTION_ENCODER
        .with(|fn_encoder| fn_encoder.borrow_mut().register_value(callback))
}

// Implement encoding for Rust callback functions

impl<R: BinaryEncode<P>, P, F> BinaryEncode<RustCallbackMarker<(P,)>> for F
where
    F: FnMut() -> R + 'static,
{
    fn encode(mut self, encoder: &mut EncodedData) {
        let value = register_callback(RustValue::new(
            move |_: &mut DecodedData, encoder: &mut EncodedData| {
                let result = (self)();
                result.encode(encoder);
            },
        ));

        encoder.push_u64(value.data().as_ffi());
    }
}

impl<R: TypeConstructor<P>, P, F> TypeConstructor<RustCallbackMarker<(P,)>> for F
where
    F: FnMut() -> R + 'static,
{
    fn create_type_instance() -> String {
        format!("new window.CallbackType({})", R::create_type_instance())
    }
}
