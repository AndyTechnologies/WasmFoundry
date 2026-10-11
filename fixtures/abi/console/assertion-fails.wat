;; A failing assertion reports and does not stop the guest: the log after it still runs.
(module
  (import "env" "console.assert" (func $assert (param i32 i32 i32)))
  (import "env" "console.log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "the count was never five")
  (func (export "_start")
    i32.const 0
    i32.const 0
    i32.const 24
    call $assert
    i32.const 0
    i32.const 0
    call $log))
