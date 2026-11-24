//! Guest module compiled from Rust to WebAssembly.
//!
//! The entry point `_start` returns 7, which `wf run` will report.

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    7
}