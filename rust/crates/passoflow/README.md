# passoflow

The stable Rust entry point for PassoFlow's local automation contracts.

This umbrella crate currently re-exports [`passoflow-core`]. It provides the
platform-independent scenario model, validation, normalization, execution
plan, diagnostics, and action/event contract without pulling in OS-specific
input, capture, or browser dependencies.

```toml
[dependencies]
passoflow = "0.1.3"
```

The lower-level `passoflow-core` crate remains available for applications that
prefer the explicit contract dependency. Additional PassoFlow crates will be
added behind stable public boundaries rather than being bundled prematurely.

License: MIT OR Apache-2.0
