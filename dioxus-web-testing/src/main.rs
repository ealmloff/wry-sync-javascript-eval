use dioxus::prelude::*;

fn main() {
    wry_testing::run(|| async {
        app();
        std::future::pending::<()>().await;
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
