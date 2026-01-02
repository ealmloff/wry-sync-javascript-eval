use dioxus::prelude::*;

fn main() {
    wry_testing::run(|| async {
        app();
    })
    .unwrap();
}

fn app() {
    launch(|| {
        rsx! {
            div { "Hello, world!" }
        }
    })
}
