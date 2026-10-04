;; A module that exports something, but not `_start`. Running it with the default entry
;; point must report WF005 naming the export it could not find, not a generic failure.
(module
  (func (export "other")))
