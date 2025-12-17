mod add_number_js;
mod callbacks;

fn main() {
    wry_testing::run(|| {
        add_number_js::test_add_number_js();
        add_number_js::test_add_number_js_batch();
        callbacks::test_call_callback();
        callbacks::test_call_callback_async();
    }).unwrap();
}
