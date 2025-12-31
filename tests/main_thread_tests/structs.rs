use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "export function increment_by_5(s) {
    for (let i = 0; i < 5; i++)
        s.increment();
}
export function get_count(s) {
    return s.count;
}")]
extern "C" {
    fn increment_by_5(s: &JsValue);
    fn get_count(s: &JsValue) -> i32;
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct Counter {
    count: i32,
}

#[wasm_bindgen]
impl Counter {
    #[wasm_bindgen(constructor)]
    pub fn new(count: i32) -> Counter {
        Counter { count }
    }

    #[wasm_bindgen(getter)]
    pub fn count(&self) -> i32 {
        self.count
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }
}

pub(crate) fn test_struct_bindings() {
    let counter = Counter::new(0);
    assert_eq!(counter.count(), 0);
    let as_js_value = JsValue::from(counter);
    increment_by_5(&as_js_value);
    assert_eq!(get_count(&as_js_value), 5);
}
