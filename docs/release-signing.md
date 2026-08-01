# Release Signing

Signing is designed in a later phase. Nothing in this repository is signed today.

Two rules apply now and are not deferred:

1. No `.pfx`, `.p12`, private key, certificate password, or signing token may ever be
   committed. `.gitignore` blocks the common extensions; that is a backstop, not a
   substitute for care.
2. Signing secrets must never be exposed to pull-request workflows. When a signing job
   is added it will live in a separate, protected job gated on a GitHub environment.
