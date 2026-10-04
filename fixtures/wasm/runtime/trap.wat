;; A module that faults immediately. The report must carry the reason — an unreachable
;; instruction — and not only the outermost backtrace context (WF009).
(module
  (func (export "_start") unreachable))
