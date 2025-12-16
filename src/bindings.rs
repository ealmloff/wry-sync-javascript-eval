//! DOM bindings using the wasm_bindgen macro
//!
//! This module provides DOM API bindings using the wry_bindgen macro,
//! which generates code compatible with the IPC protocol.

use wry_bindgen_macro::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    /// A DOM Element type
    pub type Element;

    /// Log a message to the console
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn console_log(msg: String);

    /// Show an alert dialog
    #[wasm_bindgen(js_name = alert)]
    pub fn alert(msg: String);

    /// Append a child element to a parent
    #[wasm_bindgen(method, js_class = "Element", js_name = appendChild)]
    pub fn append_child(this: &Element, child: Element);

    /// Set an attribute on an element
    #[wasm_bindgen(method, js_class = "Element", js_name = setAttribute)]
    pub fn set_attribute(this: &Element, attr: String, value: String);

    /// Set the text content of an element
    #[wasm_bindgen(method, setter, js_class = "Element", js_name = textContent)]
    pub fn set_text_content(this: &Element, text: String);

}

#[wasm_bindgen(inline_js = "export function add(a, b) { return a + b; }")]
extern "C" {
    /// Add two numbers
    #[wasm_bindgen(js_name = add)]
    pub fn add_numbers(a: u32, b: u32) -> u32;
}


#[wasm_bindgen(inline_js = "export function get_body() { return document.body; }")]
extern "C" {
    /// Get the document body
    #[wasm_bindgen(js_name = get_body)]
    pub fn get_body() -> Element;
}

#[wasm_bindgen(inline_js = "export function create_element(tag) { return document.createElement(tag); }")]
extern "C" {
    /// Create a new element with the given tag name
    #[wasm_bindgen(js_name = create_element)]
    pub fn create_element(tag: String) -> Element;
}

#[wasm_bindgen]
extern "C" {
    /// Add an event listener to an element
    #[wasm_bindgen(method, js_class = "Element", js_name = addEventListener)]
    pub fn add_event_listener(this: &Element, event: String, listener: Box<dyn FnMut()>);
}
