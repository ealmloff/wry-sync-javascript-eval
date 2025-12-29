//! Example application using wry-testing library

use wasm_bindgen::{Closure, wasm_bindgen};
use wry_testing::{JsValue, run};

fn main() -> wry::Result<()> {
    run(app)
}

fn app() {
    #[wasm_bindgen(inline_js = "export function calls_callback(cb, value) { return cb(value); }")]
    extern "C" {
        #[wasm_bindgen(js_name = calls_callback)]
        fn calls_callback(cb: Closure<dyn FnMut(u32) -> u32>, value: u32) -> u32;
    }

    let callback = Closure::new(Box::new(|x: u32| x + 1) as Box<dyn FnMut(u32) -> u32>);
    let result = calls_callback(callback, 10);
    assert_eq!(result, 11);
    // Test that JsValue can be returned from JS functions and checked with JsValue methods
    #[wasm_bindgen(inline_js = r#"
        export function get_undefined() { return undefined; }
        export function get_null() { return null; }
        export function get_object() { return { foo: "bar" }; }
    "#)]
    extern "C" {
        fn get_undefined() -> JsValue;
        fn get_null() -> JsValue;
        fn get_object() -> JsValue;
    }

    // Get values from JS and verify using JsValue methods
    let undef = get_undefined();
    eprintln!("[TEST] get_undefined() returned idx={:?}", undef);
    let is_undefined = undef.is_undefined();
    assert!(
        is_undefined,
        "get_undefined() should return undefined"
    );

    let null = get_null();
    eprintln!("[TEST] get_null() returned idx={:?}", null);
    assert!(null.is_null(), "get_null() should return null");
    loop {}
}
