# passoflow

Python bindings for PassoFlow's Rust scenario contracts.

```python
import json
import passoflow_python

result = json.loads(passoflow_python.validate_yaml("title: Demo\nsteps: []\n"))
print(result["valid"])
```

The package validates and normalizes scenarios locally. It also exposes the
opt-in Rust DOM backend for a local Chromium DevTools WebSocket endpoint:

```python
import json
from passoflow_python import DomBrowser

browser = DomBrowser("ws://127.0.0.1:9222/devtools/page/<target-id>")
browser.navigate("https://example.test")
print(json.loads(browser.preview_selector("#submit"))["count"])
```

The DOM backend is synchronous and local-only. Screen input and capture remain
separate PassoFlow-owned adapter surfaces.

For editor or integration discovery, `action_schema_json()` returns the
versioned action names, required and optional parameters, and reserved
step-metadata keys.

`input_platform_info_json()` reports whether native desktop input is supported
on the current platform and includes display, keyboard-layout, and permission
diagnostics without sending input. The result is JSON so a setup screen or
launcher can show the same status without importing platform-specific Rust
types.

See the [PassoFlow repository](https://github.com/kent-tokyo/passoflow) for
the full contract and development instructions.
