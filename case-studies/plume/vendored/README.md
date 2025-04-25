# Vendored Dependencies

This directory was added by the Paralegal developers. It vendors dependencies of
plume-models that are broken.

pulldown-cmark and rocket_csrf were git dependencies that the Plume developers
hosted on their own domain, which is now defunkt. Both sources present here were
pulled from the Plume website before it went offline.

chomp was some strange version that activated a "std" feature in its "conv"
dependency that did not exist.