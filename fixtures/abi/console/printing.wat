;; The smallest guest that proves the portable console ABI end to end.
;;
;; `env.console.log` takes a pointer and a length, in that order, and the bytes are
;; UTF-8. Both are asserted by crates/wf-cli/tests/console_abi.rs against the real binary.
(module
  (import "env" "console.log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "hello from the guest")
  (func (export "_start")
    i32.const 0
    i32.const 20
    call $log))
