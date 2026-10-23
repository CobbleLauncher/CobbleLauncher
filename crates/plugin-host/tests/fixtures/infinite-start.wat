;; Original synthetic guest start loop, used to test fuel and cancellation.
(component
  (core module $m (func $f (loop br 0)) (start $f))
  (core instance (instantiate $m)))
