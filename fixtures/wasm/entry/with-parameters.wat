;; `_start` exists as a function but takes an argument. Invoking it would mean guessing
;; what to pass, so it is rejected as WF010 with the signature it found.
(module
  (func (export "_start") (param i32)))
