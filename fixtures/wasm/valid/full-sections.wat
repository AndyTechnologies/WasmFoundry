;; One of every entity the report must show: imported function and memory, exported
;; memory, table, global and function.
(module
  (import "env" "console_log" (func (param i32 i32)))
  (import "env" "host_memory" (memory 2 16))
  (memory (export "memory") 2 16)
  (table (export "table") 1 10 funcref)
  (global (export "answer") i32 (i32.const 42))
  (func (export "_start") (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add))
