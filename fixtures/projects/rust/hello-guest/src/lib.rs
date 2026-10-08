//! Guest fixture: a Rust module with a single entry point.
//!
//! It cannot print anything yet — there is no host ABI until PHASE 8 — so its only
//! observable behaviour is the value it returns, which `wf run` reports.

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    7
}
