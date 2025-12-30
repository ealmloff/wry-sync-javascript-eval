I am developing a replacement for wasm bindgen in the wry-bindgen folder. Status:
- [x] Minimal features in web-sys compiling with the new bindgen
- [ ] Web-sys compiling with --all-features
  - [ ] Support for Clamped type
  - [ ] Support for Option<Vec<T>>
- [x] Js-sys compiling with the new bindgen
- [x] Basic roundtrip tests passing
- [ ] Casting and type checking