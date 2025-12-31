use base64::Engine;
use std::cell::RefCell;
use std::rc::Rc;

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, RequestAsyncResponder, WebViewBuilder};

use wasm_bindgen::ipc::{DecodedVariant, IPCMessage, MessageType, decode_data};
use wasm_bindgen::runtime::{AppEvent, get_runtime};

use crate::FunctionRegistry;
use crate::home::root_response;

fn decode_request_data(request: &wry::http::Request<Vec<u8>>) -> Option<IPCMessage> {
    if let Some(header_value) = request.headers().get("dioxus-data") {
        return decode_data(header_value.as_bytes());
    }
    None
}

enum WebviewLoadingState {
    Pending { queued: Vec<IPCMessage> },
    Loaded,
}

impl Default for WebviewLoadingState {
    fn default() -> Self {
        WebviewLoadingState::Pending { queued: Vec::new() }
    }
}

pub(crate) struct State {
    function_registry: &'static FunctionRegistry,
    window: Option<Window>,
    webview: Option<wry::WebView>,
    shared: Rc<RefCell<SharedWebviewState>>,
    state: WebviewLoadingState,
    proxy: EventLoopProxy<AppEvent>,
    headless: bool,
}

impl State {
    pub fn new(
        function_registry: &'static FunctionRegistry,
        proxy: EventLoopProxy<AppEvent>,
        headless: bool,
    ) -> Self {
        Self {
            function_registry,
            window: None,
            webview: None,
            shared: Rc::new(RefCell::new(SharedWebviewState::default())),
            state: WebviewLoadingState::default(),
            proxy,
            headless,
        }
    }
}

impl ApplicationHandler<AppEvent> for State {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(LogicalSize::new(800, 800).into());
        attributes.visible = !self.headless;
        let window = event_loop.create_window(attributes).unwrap();
        let shared = self.shared.clone();
        let proxy = self.proxy.clone();

        let webview = WebViewBuilder::new()
            .with_devtools(true)
            .with_asynchronous_custom_protocol("wry".into(), move |_, request, responder| {
                // path is the string slice, request is the Request object
                let real_path = request.uri().to_string().replace("wry://", "");
                let real_path = real_path.as_str().trim_matches('/');

                if real_path == "index" {
                    responder.respond(root_response());
                    return;
                }

                if real_path == "ready" {
                    proxy.send_event(AppEvent::WebviewLoaded).unwrap();
                    responder.respond(blank_response());
                    return;
                }

                // Serve inline_js modules from snippets/
                if real_path.starts_with("snippets/") {
                    if let Some(content) = crate::FUNCTION_REGISTRY.get_module(real_path) {
                        responder.respond(module_response(content));
                        return;
                    }
                    responder.respond(not_found_response());
                    return;
                }

                let mut shared = shared.borrow_mut();
                let Some(msg) = decode_request_data(&request) else {
                    responder.respond(error_response());
                    return;
                };
                if real_path == "handler" {
                    let msg_type = msg.ty().unwrap();
                    match msg_type {
                        MessageType::Evaluate => {
                            // New call from JS - save responder and wait for Rust to respond
                            shared.set_ongoing_request(responder);
                        }
                        MessageType::Respond => {
                            // JS is sending a result back to Rust
                            if shared.is_in_conversation() {
                                // We're in the middle of a conversation (Rust sent an Evaluate,
                                // JS processed it, now sending the result). Save responder and
                                // wait for Rust to send the next message (another Evaluate or
                                // the final Respond).
                                if shared.pending_evaluates > 0 {
                                    shared.pending_evaluates -= 1;
                                }
                                shared.set_ongoing_request(responder);
                            } else {
                                // No active conversation - this is the final result.
                                // Respond immediately with blank.
                                responder.respond(blank_response());
                            }
                        }
                    }
                    get_runtime().queue_rust_call(msg);
                    return;
                }

                responder.respond(blank_response());
            })
            .with_url("wry://index")
            .build_as_child(&window)
            .unwrap();

        webview.open_devtools();
        let script = self.function_registry.script();
        webview.evaluate_script(script).unwrap();

        self.window = Some(window);
        self.webview = Some(webview);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                let window = self.window.as_ref().unwrap();
                let webview = self.webview.as_ref().unwrap();

                let size = size.to_logical::<u32>(window.scale_factor());
                webview
                    .set_bounds(Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(size.width, size.height).into(),
                    })
                    .unwrap();
            }
            WindowEvent::CloseRequested => {
                std::process::exit(0);
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Shutdown(status) => {
                event_loop.exit();
                std::process::exit(status);
            }
            AppEvent::Ipc(ipc_msg) => {
                if let WebviewLoadingState::Pending { .. } = &self.state {
                    if let WebviewLoadingState::Pending { queued } = &mut self.state {
                        queued.push(ipc_msg);
                    }
                    return;
                }

                let mut shared = self.shared.borrow_mut();

                if shared.has_pending_request() {
                    shared.respond_to_request(ipc_msg);
                    return;
                }

                let decoded = ipc_msg.decoded().unwrap();

                if let DecodedVariant::Evaluate { .. } = decoded {
                    // Encode the binary data as base64 and pass to JS
                    // JS will iterate over operations in the buffer
                    let engine = base64::engine::general_purpose::STANDARD;
                    let data_base64 = engine.encode(ipc_msg.data());
                    let code = format!("window.evaluate_from_rust_binary(\"{}\")", data_base64);
                    self.webview
                        .as_ref()
                        .unwrap()
                        .evaluate_script(&code)
                        .unwrap();
                }
            }
            AppEvent::WebviewLoaded => {
                if let WebviewLoadingState::Pending { queued } =
                    std::mem::replace(&mut self.state, WebviewLoadingState::Loaded)
                {
                    for msg in queued {
                        get_runtime().js_response(msg);
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
        }
    }
}

#[derive(Default)]
struct SharedWebviewState {
    ongoing_request: Option<RequestAsyncResponder>,
    /// Count of Evaluates we've sent to JS that we're still waiting for responses to
    pending_evaluates: usize,
}

impl SharedWebviewState {
    fn set_ongoing_request(&mut self, responder: RequestAsyncResponder) {
        self.ongoing_request = Some(responder);
    }

    fn take_ongoing_request(&mut self) -> Option<RequestAsyncResponder> {
        self.ongoing_request.take()
    }

    fn has_pending_request(&self) -> bool {
        self.ongoing_request.is_some()
    }

    fn is_in_conversation(&self) -> bool {
        self.has_pending_request() || self.pending_evaluates > 0
    }

    fn respond_to_request(&mut self, response: IPCMessage) {
        if let Some(responder) = self.take_ongoing_request() {
            let ty = response.ty().unwrap();
            if ty == MessageType::Evaluate {
                self.pending_evaluates += 1;
            }

            let body = response.into_data();
            responder.respond(
                wry::http::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/octet-stream")
                    .body(body)
                    .expect("Failed to build response"),
            );
        }
    }
}

fn error_response() -> wry::http::Response<Vec<u8>> {
    wry::http::Response::builder()
        .status(400)
        .body(vec![])
        .expect("Failed to build error response")
}

fn blank_response() -> wry::http::Response<Vec<u8>> {
    wry::http::Response::builder()
        .status(200)
        .body(vec![])
        .expect("Failed to build blank response")
}

fn module_response(content: &str) -> wry::http::Response<Vec<u8>> {
    wry::http::Response::builder()
        .status(200)
        .header("Content-Type", "application/javascript")
        .body(content.as_bytes().to_vec())
        .expect("Failed to build module response")
}

fn not_found_response() -> wry::http::Response<Vec<u8>> {
    wry::http::Response::builder()
        .status(404)
        .body(b"Not Found".to_vec())
        .expect("Failed to build not found response")
}
