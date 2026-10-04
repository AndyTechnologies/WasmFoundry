;; Imports nothing the runtime can provide yet: there is no host ABI until PHASE 8, so
;; every import fails to resolve (WF002).
(module
  (import "env" "log" (func))
  (func (export "_start")))
