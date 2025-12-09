use std::marker::PhantomData;
use std::sync::{Arc, OnceLock, RwLock, mpsc};
use std::thread;

use base64::Engine;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use slotmap::{DefaultKey, Key, KeyData, SlotMap};
use winit::event_loop::EventLoopProxy;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebViewBuilder};

// Message types for thread communication
#[derive(Debug)]
enum JSThreadMessage {
    Evaluate {
        code: String,
        result_sender: mpsc::Sender<Vec<u8>>,
    },
    Shutdown,
}

#[derive(Default)]
struct State {
    window: Option<Window>,
    webview: Option<wry::WebView>,
    pending_requests: Arc<RwLock<Vec<mpsc::Sender<Vec<u8>>>>>,
}

impl ApplicationHandler<JSThreadMessage> for State {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes();
        attributes.inner_size = Some(LogicalSize::new(800, 800).into());
        let window = event_loop.create_window(attributes).unwrap();
        let pending_requests = self.pending_requests.clone();

        let webview = WebViewBuilder::new()
            .with_custom_protocol("wry".into(), move |_, request| {
                // path is the string slice, request is the Request object
                let real_path = request.uri().to_string().replace("wry://", "");
                let real_path = real_path.as_str().trim_matches('/');
                println!("Handling request for path: {}", real_path);
                println!("Request: {:?}", request);
                if real_path == "handler" {
                    if let Some(queued) = pending_requests.write().unwrap().pop() {
                        if let Some(header_value) = request.headers().get("dioxus-data") {
                            println!("Received header value: {:?}", header_value);
                            // Decode base64 header
                            let engine = base64::engine::general_purpose::STANDARD;
                            if let Ok(decoded_bytes) = engine.decode(header_value) {
                                let _ = queued.send(decoded_bytes);
                            }
                        }
                    }

                    // Set the correct origin based on platform
                    let origin = if cfg!(target_os = "windows") || cfg!(target_os = "android") {
                        "http://wry.local"
                    } else {
                        "wry://local"
                    };

                    wry::http::Response::builder()
                        .header("Content-Type", "application/xml")
                        .header("Access-Control-Allow-Origin", origin)
                        .header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
                        .header("Access-Control-Allow-Headers", "Content-Type")
                        .body(vec![].into())
                        .map_err(|e| e.to_string())
                        .expect("Failed to build response")
                }
                else if real_path == "callback" {
                        if let Some(header_value) = request.headers().get("dioxus-data") {
                            println!("Received header value for callback: {:?}", header_value);
                            // Decode base64 header
                            let engine = base64::engine::general_purpose::STANDARD;
                            if let Ok(decoded_bytes) = engine.decode(header_value) {
                                #[derive(Deserialize)]
                                struct FunctionCall {
                                    code: u64,
                                    args: Vec<serde_json::Value>,
                                }
                                let function_call: FunctionCall = serde_json::from_slice(&decoded_bytes).unwrap();
                                let mut encoder = EVENT_LOOP_PROXY
                                    .get()
                                    .expect("Event loop proxy not set")
                                    .encoder
                                    .write()
                                    .unwrap();
                                if let Some(func) = encoder.functions.get_mut(KeyData::from_ffi(function_call.code).into()) {
                                    let result = func(function_call.args);
                                    let serialized_result = serde_json::to_vec(&result).unwrap();
                                    return wry::http::Response::builder()
                                        .header("Content-Type", "application/xml")
                                        .body(serialized_result.into())
                                        .map_err(|e| e.to_string())
                                        .expect("Failed to build response");
                                }
                            }
                        }

                    wry::http::Response::builder()
                        .header("Content-Type", "application/xml")
                        .body(vec![].into())
                        .map_err(|e| e.to_string())
                        .expect("Failed to build response")
                }
                else {
                    // Serve the main HTML page
                    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Wry Test</title>
</head>
<body>
    <h1>Wry Custom Protocol Test</h1>
    <button onclick="testRequest()">Test XML Request</button>
    <div id="result"></div>

    <script>
        function testRequest() {
            sync_request("wry://handler", { event: "test_event" })
                .then(response => {
                    document.getElementById("result").innerText = "Response: " + JSON.stringify(response);
                });
        }
        // This function sends the event to the virtualdom and then waits for the virtualdom to process it
        //
        // However, it's not really suitable for liveview, because it's synchronous and will block the main thread
        // We should definitely consider using a websocket if we want to block... or just not block on liveview
        // Liveview is a little bit of a tricky beast
        function sync_request(endpoint, contents) {
            // Handle the event on the virtualdom and then process whatever its output was
            const xhr = new XMLHttpRequest();

            // Serialize the event and send it to the custom protocol in the Rust side of things
            xhr.open("POST", endpoint, false);
            xhr.setRequestHeader("Content-Type", "application/json");

            // hack for android since we CANT SEND BODIES (because wry is using shouldInterceptRequest)
            //
            // https://issuetracker.google.com/issues/119844519
            // https://stackoverflow.com/questions/43273640/android-webviewclient-how-to-get-post-request-body
            // https://developer.android.com/reference/android/webkit/WebViewClient#shouldInterceptRequest(android.webkit.WebView,%20android.webkit.WebResourceRequest)
            //
            // the issue here isn't that big, tbh, but there's a small chance we lose the event due to header max size (16k per header, 32k max)
            const json_string = JSON.stringify(contents);
            const contents_bytes = new TextEncoder().encode(json_string);
            const contents_base64 = btoa(String.fromCharCode.apply(null, contents_bytes));
            xhr.setRequestHeader("dioxus-data", contents_base64);
            xhr.send();
        }

        function run_code(code, args) {
            let f;
            switch (code) {
                case 0:
                    f = console.log;
                    break;
                case 1:
                    f = alert;
                    break;
                case 2:
                    f = function(a, b) { return a + b; };
                    break;
                case 3:
                    f = function(event_name, callback) {
                        document.addEventListener(event_name, function(e) {
                            callback.call();
                        });
                    };
                    break;
                default:
                    throw new Error("Unknown code: " + code);
            }
            return f.apply(null, args);
        }

        function evaluate_from_rust(code, args_json) {
            let args = deserialize_args(args_json);
            const result = run_code(code, args);
            sync_request("wry://handler", result);
        }

        function deserialize_args(args_json) {
            if (typeof args_json === "string") {
                return args_json;
            } else if (typeof args_json === "number") {
                return args_json;
            } else if (Array.isArray(args_json)) {
                return args_json.map(deserialize_args);
            } else if (typeof args_json === "object" && args_json !== null) {
                if (args_json.type === "function") {
                    return new RustFunction(args_json.id);
                } else {
                    const obj = {};
                    for (const key in args_json) {
                        obj[key] = deserialize_args(args_json[key]);
                    }
                    return obj;
                }
            }
        }

        class RustFunction {
            constructor(code) {
                this.code = code;
            }

            call(...args) {
                return sync_request("wry://callback", {
                    code: this.code,
                    args: args
                });
            }
        }
    </script>
</body>
</html>"#;

                    wry::http::Response::builder()
                        .header("Content-Type", "text/html")
                        .body(html.as_bytes().into())
                        .map_err(|e| e.to_string())
                        .expect("Failed to build response")
                }
            })
            .with_url("wry://local")
            .build_as_child(&window)
            .unwrap();

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
            _ => {
                // println!("got event\n{:#?}", event);
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: JSThreadMessage) {
        match event {
            JSThreadMessage::Evaluate {
                code,
                result_sender,
            } => {
                self.pending_requests.write().unwrap().push(result_sender);
                self.webview
                    .as_ref()
                    .unwrap()
                    .evaluate_script(&code)
                    .unwrap();
            }
            JSThreadMessage::Shutdown => {
                event_loop.exit();
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

struct DomEnv {
    encoder: RwLock<Encoder>,
    proxy: EventLoopProxy<JSThreadMessage>,
}

static EVENT_LOOP_PROXY: OnceLock<DomEnv> = OnceLock::new();

fn set_event_loop_proxy(proxy: EventLoopProxy<JSThreadMessage>) {
    EVENT_LOOP_PROXY
        .set(DomEnv {
            encoder: RwLock::new(Encoder::new()),
            proxy,
        })
        .unwrap_or_else(|_| panic!("Event loop proxy already set"));
}

fn get_event_loop_proxy() -> &'static EventLoopProxy<JSThreadMessage> {
    &EVENT_LOOP_PROXY
        .get()
        .expect("Event loop proxy not set")
        .proxy
}

fn main() -> wry::Result<()> {
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    {
        use gtk::prelude::DisplayExtManual;

        gtk::init().unwrap();
        if gtk::gdk::Display::default().unwrap().backend().is_wayland() {
            panic!("This example doesn't support wayland!");
        }

        winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
            let error = error as *mut x11_dl::xlib::ErrorEvent;
            (unsafe { (*error).error_code }) == 170
        }));
    }

    let event_loop = EventLoop::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    set_event_loop_proxy(proxy);
    std::thread::spawn(app);
    let mut state = State::default();
    event_loop.run_app(&mut state).unwrap();

    Ok(())
}

fn app() {
    let add_function = ADD_NUMBERS;
    let assert_sum_works = move || {
        let sum: i32 = add_function.call(5, 7);
        println!("Sum from JS: {}", sum);
        assert_eq!(sum, 12);
    };
    assert_sum_works();
    let add_event_listener: JSFunction<fn(_, _)> = JSFunction::new(3);
    add_event_listener.call("click".to_string(), move || {
        println!("Button clicked!");
        assert_sum_works();
    });
}

struct Encoder {
    functions: SlotMap<
        DefaultKey,
        Box<dyn FnMut(Vec<serde_json::Value>) -> serde_json::Value + Send + Sync>,
    >,
}

impl Encoder {
    fn new() -> Self {
        Self {
            functions: SlotMap::new(),
        }
    }

    fn encode<T: RustEncode<P>, P>(&mut self, value: T) -> serde_json::Value {
        value.encode(self)
    }

    fn encode_function<T: IntoRustCallable<P>, P>(&mut self, function: T) -> serde_json::Value {
        let key = self.functions.insert(function.into());
        serde_json::json!({
            "type": "function",
            "id": key.data().as_ffi(),
        })
    }
}

trait RustEncode<P = ()> {
    fn encode(self, encoder: &mut Encoder) -> serde_json::Value;
}

impl RustEncode for String {
    fn encode(self, _encoder: &mut Encoder) -> serde_json::Value {
        serde_json::Value::String(self)
    }
}

impl RustEncode for () {
    fn encode(self, _encoder: &mut Encoder) -> serde_json::Value {
        serde_json::Value::Null
    }
}

impl RustEncode for i32 {
    fn encode(self, _encoder: &mut Encoder) -> serde_json::Value {
        serde_json::Value::Number(serde_json::Number::from(self))
    }
}

impl<F, P> RustEncode<P> for F
where
    F: IntoRustCallable<P>,
{
    fn encode(self, encoder: &mut Encoder) -> serde_json::Value {
        encoder.encode_function(self)
    }
}

trait IntoRustCallable<T> {
    fn into(self) -> Box<dyn FnMut(Vec<serde_json::Value>) -> serde_json::Value + Send + Sync>;
}

impl<R, F> IntoRustCallable<fn() -> R> for F
where
    F: FnMut() -> R + Send + Sync + 'static,
    R: serde::Serialize,
{
    fn into(mut self) -> Box<dyn FnMut(Vec<serde_json::Value>) -> serde_json::Value + Send + Sync> {
        Box::new(move |args: Vec<serde_json::Value>| {
            let result: R = (self)();
            serde_json::to_value(result).unwrap()
        })
    }
}

impl<T, R, F> IntoRustCallable<fn(T) -> R> for F
where
    F: FnMut(T) -> R + Send + Sync + 'static,
    T: for<'de> Deserialize<'de>,
    R: serde::Serialize,
{
    fn into(mut self) -> Box<dyn FnMut(Vec<serde_json::Value>) -> serde_json::Value + Send + Sync> {
        Box::new(move |args: Vec<serde_json::Value>| {
            let mut args_iter = args.into_iter();
            let arg: T = serde_json::from_value(args_iter.next().unwrap()).unwrap();
            let result: R = (self)(arg);
            serde_json::to_value(result).unwrap()
        })
    }
}

impl<T1, T2, R, F> IntoRustCallable<fn(T1, T2) -> R> for F
where
    F: FnMut(T1, T2) -> R + Send + Sync + 'static,
    T1: for<'de> Deserialize<'de>,
    T2: for<'de> Deserialize<'de>,
    R: serde::Serialize,
{
    fn into(mut self) -> Box<dyn FnMut(Vec<serde_json::Value>) -> serde_json::Value + Send + Sync> {
        Box::new(move |args: Vec<serde_json::Value>| {
            let mut args_iter = args.into_iter();
            let arg1: T1 = serde_json::from_value(args_iter.next().unwrap()).unwrap();
            let arg2: T2 = serde_json::from_value(args_iter.next().unwrap()).unwrap();
            let result: R = (self)(arg1, arg2);
            serde_json::to_value(result).unwrap()
        })
    }
}

const CONSOLE_LOG: JSFunction<fn(String)> = JSFunction::new(0);
const ALERT: JSFunction<fn(String)> = JSFunction::new(1);
const ADD_NUMBERS: JSFunction<fn(i32, i32) -> i32> = JSFunction::new(2);
const ADD_EVENT_LISTENER: JSFunction<fn(String, fn())> = JSFunction::new(3);

struct JSFunction<T> {
    id: u32,
    function: PhantomData<T>,
}

impl<T> JSFunction<T> {
    const fn new(id: u32) -> Self {
        Self {
            id,
            function: PhantomData,
        }
    }
}

impl<T, R> JSFunction<fn(T) -> R> {
    fn call<P>(&self, args: T) -> R
    where
        T: RustEncode<P>,
        R: DeserializeOwned,
    {
        let mut encoder = EVENT_LOOP_PROXY
            .get()
            .expect("Event loop proxy not set")
            .encoder
            .write()
            .unwrap();
        let args_json = encoder.encode(args);
        let code = format_call(self.id, std::iter::once(args_json));
        run_js_sync(get_event_loop_proxy(), code)
    }
}

impl<T1, T2, R> JSFunction<fn(T1, T2) -> R> {
    fn call<P1, P2>(&self, arg1: T1, arg2: T2) -> R
    where
        T1: RustEncode<P1>,
        T2: RustEncode<P2>,
        R: DeserializeOwned,
    {
        let mut encoder = EVENT_LOOP_PROXY
            .get()
            .expect("Event loop proxy not set")
            .encoder
            .write()
            .unwrap();
        let arg1_json = encoder.encode(arg1);
        let arg2_json = encoder.encode(arg2);
        let code = format_call(self.id, [arg1_json, arg2_json].into_iter());
        run_js_sync(get_event_loop_proxy(), code)
    }
}

fn format_call<'a>(function_id: u32, args: impl Iterator<Item = serde_json::Value>) -> String {
    let mut call = String::new();
    call.push_str("evaluate_from_rust(");
    call.push_str(&function_id.to_string());
    call.push_str(", [");
    for (i, arg) in args.enumerate() {
        if i > 0 {
            call.push_str(", ");
        }
        call.push_str(&arg.to_string());
    }
    call.push_str("])");
    call
}

fn run_js_sync<T: DeserializeOwned>(proxy: &EventLoopProxy<JSThreadMessage>, code: String) -> T {
    let (result_sender, result_receiver) = mpsc::channel();
    proxy
        .send_event(JSThreadMessage::Evaluate {
            code,
            result_sender,
        })
        .unwrap();

    let mut result_bytes = result_receiver.recv().unwrap();
    if result_bytes.is_empty() {
        result_bytes = b"null".to_vec();
    }
    serde_json::from_slice(&result_bytes).unwrap()
}
