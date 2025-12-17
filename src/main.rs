use std::fmt::Write;
use std::sync::LazyLock;
use winit::event_loop::EventLoop;

use wry_bindgen::prelude::*;

use crate::{bindings::WINDOW, webview::State};

inventory::collect!(JsFunctionSpec);

pub mod bindings;
mod home;
mod webview;

pub struct JsFunctionSpec {
    pub name: &'static str,
    pub js_code: &'static str,
    pub type_info: fn() -> (Vec<String>, String),
    /// Optional inline JS module content (ES module with exports)
    pub inline_js: Option<InlineJsModule>,
}

/// Inline JS module info
pub struct InlineJsModule {
    /// The JS module content
    pub content: &'static str,
    /// The exported function name (js_name)
    pub export_name: &'static str,
}

impl JsFunctionSpec {
    pub const fn new(
        name: &'static str,
        js_code: &'static str,
        type_info: fn() -> (Vec<String>, String),
        inline_js: Option<InlineJsModule>,
    ) -> Self {
        Self {
            name,
            js_code,
            type_info,
            inline_js,
        }
    }
}

impl InlineJsModule {
    pub const fn new(content: &'static str, export_name: &'static str) -> Self {
        Self {
            content,
            export_name,
        }
    }
}

// Re-export bindings for convenience
pub use bindings::{Element, alert, console_log};

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
    let registry = &*FUNCTION_REGISTRY;

    std::thread::spawn(app);
    let mut state = State::new(registry);
    event_loop.run_app(&mut state).unwrap();

    Ok(())
}

pub struct FunctionRegistry {
    functions: String,
    function_ids: Vec<JsFunctionId>,
    /// Map of module path -> module content for inline_js modules
    modules: std::collections::HashMap<String, &'static str>,
}

struct JsFunctionId {
    name: &'static str,
}

pub static FUNCTION_REGISTRY: LazyLock<FunctionRegistry> =
    LazyLock::new(FunctionRegistry::collect_from_inventory);

impl FunctionRegistry {
    fn collect_from_inventory() -> Self {
        let mut function_ids = Vec::new();
        let mut modules = std::collections::HashMap::new();

        // First pass: collect all specs and module info
        let specs: Vec<_> = inventory::iter::<JsFunctionSpec>().collect();

        for spec in &specs {
            let id = JsFunctionId { name: spec.name };
            function_ids.push(id);

            // Store module content for serving via custom protocol
            if let Some(ref inline_js) = spec.inline_js {
                let module_path = format!("snippets/{}.js", spec.name);
                modules.insert(module_path, inline_js.content);
            }
        }

        // Build the script - inline module content directly to avoid async issues
        let mut script = String::new();
        script.push_str("window.__wryModules = {};\n");

        // Inline each module's content using IIFE to create module-like scope
        for spec in &specs {
            if let Some(ref inline_js) = spec.inline_js {
                // Convert "export function foo" to "function foo" and capture exports
                let module_code = inline_js.content.replace("export ", "");
                write!(
                    &mut script,
                    "window.__wryModules[\"{}\"] = (() => {{ {}; return {{ {} }}; }})();\n",
                    spec.name, module_code, inline_js.export_name
                )
                .unwrap();
            }
        }

        // Now set up the function registry
        script.push_str("window.setFunctionRegistry([");
        for (i, spec) in specs.iter().enumerate() {
            if i > 0 {
                script.push_str(",\n");
            }
            let (args, return_type) = (spec.type_info)();
            write!(
                &mut script,
                "window.createWrapperFunction([{}], {}, {})",
                args.join(", "),
                return_type,
                spec.js_code
            )
            .unwrap();
        }
        script.push_str("]);");

        Self {
            functions: script,
            function_ids,
            modules,
        }
    }

    fn get_function<F>(&self, name: &str) -> Option<JSFunction<F>>
    where
        F: 'static,
    {
        for (i, id) in self.function_ids.iter().enumerate() {
            if id.name == name {
                return Some(JSFunction::new(i as u32));
            }
        }
        None
    }

    pub(crate) fn script(&self) -> &str {
        &self.functions
    }

    /// Get the content of an inline_js module by path
    pub fn get_module(&self, path: &str) -> Option<&'static str> {
        self.modules.get(path).copied()
    }
}

fn app() {
    batch(|| {
        // Get document body using the lazily-initialized WINDOW static
        let document = WINDOW.with(|window| window.document());
        let body = document.body();

        // Create a container div
        let container = document.create_element("div".to_string());
        container.set_attribute("id".to_string(), "heap-demo".to_string());
        container.set_attribute("style".to_string(),
        "margin: 20px; padding: 15px; border: 2px solid #4CAF50; border-radius: 8px; background: #f9f9f9;".to_string());

        // Create a heading
        let heading = document.create_element("h2".to_string());
        heading.set_text_content("JSHeap Demo".to_string());
        heading.set_attribute(
            "style".to_string(),
            "color: #333; margin-top: 0;".to_string(),
        );
        container.append_child(heading);

        // Create a counter display
        let counter_display = document.create_element("p".to_string());
        counter_display.set_attribute("id".to_string(), "heap-counter".to_string());
        counter_display.set_attribute(
            "style".to_string(),
            "font-size: 24px; font-weight: bold; color: #2196F3;".to_string(),
        );
        counter_display.set_text_content("Counter: 0".to_string());
        container.append_child(counter_display.clone());

        // Create a button
        let button = document.create_element("button".to_string());
        button.set_text_content("Click me (heap-managed)".to_string());
        button.set_attribute("id".to_string(), "heap-button".to_string());
        button.set_attribute("style".to_string(),
        "padding: 10px 20px; font-size: 16px; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px;".to_string());
        container.append_child(button);
        // Append container to body
        body.append_child(container);

        let counter_ref = counter_display.clone();
        // Demo 4: Event handling with heap refs
        let mut count = 0;
        body.add_event_listener(
            "click".to_string(),
            Box::new(move || {
                count += 1;

                // Update the counter display using the heap ref
                let start = std::time::Instant::now();
                counter_ref.set_text_content(format!("Counter: {}", count));
                let duration = start.elapsed();
                println!(
                    "Updated counter display in {:?} microseconds",
                    duration.as_micros()
                );
            }),
        );
    });

    // Keep running to handle events
    wait_for_js_event::<()>();
}
