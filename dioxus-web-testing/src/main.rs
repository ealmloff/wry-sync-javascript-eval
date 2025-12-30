use dioxus::prelude::*;

fn main() {
    wry_testing::run(|| {
        app();
        wait_for_js_result::<i32>();
    })
    .unwrap();
}

fn app() {
    launch(|| rsx!{
        div { "Hello, world!" }
    })
}