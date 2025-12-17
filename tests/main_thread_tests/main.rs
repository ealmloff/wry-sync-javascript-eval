mod add_number_js;
mod callbacks;
mod jsvalue;

fn main() {
    wry_testing::run(|| {
        add_number_js::test_add_number_js();
        add_number_js::test_add_number_js_batch();
        callbacks::test_call_callback();
        callbacks::test_call_callback_async();

        // JsValue behavior tests
        jsvalue::test_jsvalue_constants();
        jsvalue::test_jsvalue_bool();
        jsvalue::test_jsvalue_default();
        jsvalue::test_jsvalue_clone_reserved();
        jsvalue::test_jsvalue_equality();
        jsvalue::test_jsvalue_from_js();
        jsvalue::test_jsvalue_pass_to_js();
    }).unwrap();
}
