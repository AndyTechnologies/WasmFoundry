;; One mutable and one immutable global: mutability is part of the contract a host
;; reads, not decoration.
(module
  (global (export "immutable") i32 (i32.const 1))
  (global (export "mutable") (mut i32) (i32.const 2)))
