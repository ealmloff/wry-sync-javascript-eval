use wasm_bindgen::{Closure, wasm_bindgen};

#[wasm_bindgen(inline_js = r#"
const originalLog = console.log;
const originalWarn = console.warn;
const originalError = console.error;

let onLogCallback = null;

function formatArgs(args) {
    return Array.from(args).map(arg => {
        try {
            return typeof arg === 'object' ? JSON.stringify(arg) : String(arg);
        } catch (e) {
            return String(arg);
        }
    }).join(' ');
}

console.log = function(...args) {
    originalLog.apply(console, args);
    onLogCallback && onLogCallback(formatArgs(args));
};

console.warn = function(...args) {
    originalWarn.apply(console, args);
    onLogCallback && onLogCallback('WARN: ' + formatArgs(args));
};

console.error = function(...args) {
    originalError.apply(console, args);
    onLogCallback && onLogCallback('ERROR: ' + formatArgs(args));
};

export function set_on_log(callback) {
    originalLog.call(console, "Setting onLogCallback");
    onLogCallback = callback;
}

export function set_on_error(callback) {
    window.addEventListener('error', function(event) {
        callback(event.message + ' at ' + event.filename + ':' + event.lineno + ':' + event.colno, event.error ? event.error.stack : '');
    });
}
"#)]
extern "C" {
    pub fn set_on_log(callback: Closure<dyn FnMut(String)>);
    pub fn set_on_error(callback: Closure<dyn FnMut(String, String)>);
}
