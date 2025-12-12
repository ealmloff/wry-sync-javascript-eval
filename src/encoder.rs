use slotmap::{DefaultKey, Key, KeyData, SlotMap};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::mpsc::Receiver;
use std::sync::{OnceLock, mpsc};
use winit::event_loop::EventLoopProxy;

use crate::DomEnv;
use crate::ipc::{IPCMessage, DecodedData, EncodedData, encode_respond};

/// A reference to a JavaScript heap object, identified by a unique ID.
/// References are encoded as u32 in the binary protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JSHeapRef {
    id: u32,
}

impl JSHeapRef {
    /// Create a new JSHeapRef from a raw ID (typically received from JS)
    #[allow(dead_code)]
    pub fn from_id(id: u32) -> Self {
        Self { id }
    }

    /// Get the raw ID of this heap reference
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Trait for encoding Rust values into the binary protocol.
/// Each type specifies how to serialize itself.
pub(crate) trait BinaryEncode {
    fn encode(&self, encoder: &mut EncodedData, fn_encoder: &mut FunctionEncoder);
}

/// Trait for decoding values from the binary protocol.
/// Each type specifies how to deserialize itself.
pub(crate) trait BinaryDecode: Sized {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()>;
}

// Implementations for basic types

impl BinaryEncode for () {
    fn encode(&self, _encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        // Unit type encodes as nothing
    }
}

impl BinaryDecode for () {
    fn decode(_decoder: &mut DecodedData) -> Result<Self, ()> {
        Ok(())
    }
}

impl BinaryEncode for bool {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u8(if *self { 1 } else { 0 });
    }
}

impl BinaryDecode for bool {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        Ok(decoder.take_u8()? != 0)
    }
}

impl BinaryEncode for u8 {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u8(*self);
    }
}

impl BinaryDecode for u8 {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        decoder.take_u8()
    }
}

impl BinaryEncode for u16 {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u16(*self);
    }
}

impl BinaryDecode for u16 {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        decoder.take_u16()
    }
}

impl BinaryEncode for u32 {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u32(*self);
    }
}

impl BinaryDecode for u32 {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        decoder.take_u32()
    }
}

impl BinaryEncode for i32 {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u32(*self as u32);
    }
}

impl BinaryDecode for i32 {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        Ok(decoder.take_u32()? as i32)
    }
}

impl BinaryEncode for String {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_str(self);
    }
}

impl BinaryDecode for String {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        Ok(decoder.take_str()?.to_string())
    }
}

impl BinaryEncode for JSHeapRef {
    fn encode(&self, encoder: &mut EncodedData, _fn_encoder: &mut FunctionEncoder) {
        encoder.push_u32(self.id);
    }
}

impl BinaryDecode for JSHeapRef {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        Ok(JSHeapRef { id: decoder.take_u32()? })
    }
}

impl<T: BinaryDecode> BinaryDecode for Option<T> {
    fn decode(decoder: &mut DecodedData) -> Result<Self, ()> {
        let has_value = decoder.take_u8()? != 0;
        if has_value {
            Ok(Some(T::decode(decoder)?))
        } else {
            Ok(None)
        }
    }
}

/// A wrapper for Rust callbacks that can be called from JS.
/// This is encoded as a u32 callback ID.
pub struct RustCallback<F> {
    callback: F,
}

impl<F> RustCallback<F> {
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

/// Marker type used in JSFunction type signatures
pub struct Callback;

/// Encoder for storing Rust callbacks that can be called from JS.
/// This is stored in a thread-local so callbacks persist across the call chain.
pub(crate) struct FunctionEncoder {
    functions: SlotMap<
        DefaultKey,
        Option<Box<dyn FnMut(&mut DecodedData) -> EncodedData + Send + Sync>>,
    >,
}

impl FunctionEncoder {
    pub(crate) fn new() -> Self {
        Self {
            functions: SlotMap::new(),
        }
    }

    fn register_function<F>(&mut self, f: F) -> u32
    where
        F: FnMut(&mut DecodedData) -> EncodedData + Send + Sync + 'static,
    {
        let key = self.functions.insert(Some(Box::new(f)));
        key.data().as_ffi() as u32
    }
}

thread_local! {
    static THREAD_LOCAL_FUNCTION_ENCODER: RefCell<FunctionEncoder> = RefCell::new(FunctionEncoder::new());
}

/// Register a callback in the thread-local function encoder
fn register_callback<F>(f: F) -> u32
where
    F: FnMut(&mut DecodedData) -> EncodedData + Send + Sync + 'static,
{
    THREAD_LOCAL_FUNCTION_ENCODER.with(|encoder| {
        encoder.borrow_mut().register_function(f)
    })
}

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
}

impl<R: BinaryDecode> JSFunction<fn() -> R> {
    pub fn call(&self, _: ()) -> R {
        let encoder = EncodedData::new();
        run_js_sync::<R>(self.id, encoder)
    }
}

impl<T1: BinaryEncode, R: BinaryDecode> JSFunction<fn(T1) -> R> {
    pub fn call(&self, arg: T1) -> R {
        THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
            let mut encoder = EncodedData::new();
            arg.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            run_js_sync::<R>(self.id, encoder)
        })
    }
}

impl<T1: BinaryEncode, T2: BinaryEncode, R: BinaryDecode> JSFunction<fn(T1, T2) -> R> {
    pub fn call(&self, arg1: T1, arg2: T2) -> R {
        THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
            let mut encoder = EncodedData::new();
            arg1.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            arg2.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            run_js_sync::<R>(self.id, encoder)
        })
    }
}

impl<T1: BinaryEncode, T2: BinaryEncode, T3: BinaryEncode, R: BinaryDecode> JSFunction<fn(T1, T2, T3) -> R> {
    pub fn call(&self, arg1: T1, arg2: T2, arg3: T3) -> R {
        THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
            let mut encoder = EncodedData::new();
            arg1.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            arg2.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            arg3.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            run_js_sync::<R>(self.id, encoder)
        })
    }
}

// Special implementation for functions that take a RustCallback using the Callback marker
impl<T1: BinaryEncode> JSFunction<fn(T1, Callback)> {
    pub fn call<F: FnMut() -> bool + Send + Sync + 'static>(&self, arg1: T1, callback: F) {
        THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
            let mut encoder = EncodedData::new();
            arg1.encode(&mut encoder, &mut fn_encoder.borrow_mut());
            
            // Register the callback and encode its ID
            let mut cb = callback;
            let callback_id = register_callback(move |_decoder| {
                let result = cb();
                let mut response = EncodedData::new();
                response.push_u8(if result { 1 } else { 0 });
                response
            });
            encoder.push_u32(callback_id);
            
            run_js_sync::<()>(self.id, encoder)
        })
    }
}

fn run_js_sync<R: BinaryDecode>(fn_id: u32, args: EncodedData) -> R {
    let proxy = &get_dom().proxy;
    let data = crate::ipc::encode_evaluate(fn_id, &args);
    
    println!("Sending JS evaluate request for fn_id: {}", fn_id);
    _ = proxy.send_event(IPCMessage::Evaluate { fn_id, data });

    wait_for_js_event()
}

pub(crate) fn wait_for_js_event<R: BinaryDecode>() -> R {
    let env = EVENT_LOOP_PROXY.get().expect("Event loop proxy not set");
    THREAD_LOCAL_RECEIVER.with(|receiver| {
        println!("Waiting for JS response...");
        while let Ok(response) = receiver.recv() {
            println!("Received response: {:?}", response);
            match response {
                IPCMessage::Respond { data } => {
                    println!("Got response from JS");
                    // Skip the message type (first u32 after header)
                    let mut decoder = DecodedData::from_bytes(&data).expect("Failed to decode response");
                    let _ = decoder.take_u32(); // Skip message type
                    return R::decode(&mut decoder).expect("Failed to decode return value");
                }
                IPCMessage::Evaluate { fn_id, data } => {
                    let key: DefaultKey = KeyData::from_ffi(fn_id as u64).into();
                    
                    // Get the function from the thread-local encoder
                    let function = THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
                        fn_encoder
                            .borrow_mut()
                            .functions
                            .get_mut(key)
                            .and_then(|f| f.take())
                    });
                    
                    if let Some(mut function) = function {
                        let mut decoder = DecodedData::from_bytes(&data).expect("Failed to decode");
                        let _ = decoder.take_u32(); // Skip message type
                        let _ = decoder.take_u32(); // Skip fn_id
                        
                        let result = function(&mut decoder);
                        println!("Evaluated function in Rust, sending response back to JS");
                        let response_data = encode_respond(&result);
                        env.js_response(IPCMessage::Respond { data: response_data });
                        
                        // Insert it back into the thread-local encoder
                        THREAD_LOCAL_FUNCTION_ENCODER.with(|fn_encoder| {
                            fn_encoder.borrow_mut().functions.get_mut(key).unwrap().replace(function);
                        });
                    }
                }
                IPCMessage::Shutdown => {
                    panic!("Shutdown received")
                }
            }
        }
        panic!("Channel closed")
    })
}

thread_local! {
    static THREAD_LOCAL_RECEIVER: Receiver<IPCMessage> = {
        let env = EVENT_LOOP_PROXY.get().expect("Event loop proxy not set");
        let (sender, receiver) = mpsc::channel();
        env.set_sender(sender);
        receiver
    };
}

static EVENT_LOOP_PROXY: OnceLock<DomEnv> = OnceLock::new();

pub(crate) fn set_event_loop_proxy(proxy: EventLoopProxy<IPCMessage>) {
    EVENT_LOOP_PROXY
        .set(DomEnv::new(proxy))
        .unwrap_or_else(|_| panic!("Event loop proxy already set"));
}

pub(crate) fn get_dom() -> &'static DomEnv {
    EVENT_LOOP_PROXY.get().expect("Event loop proxy not set")
}
