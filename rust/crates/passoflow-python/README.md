# passoflow

Python bindings for PassoFlow's Rust scenario contracts.

```python
import json
import passoflow_python

result = json.loads(passoflow_python.validate_yaml("title: Demo\nsteps: []\n"))
print(result["valid"])
```

The package validates and normalizes scenarios locally. Desktop input, screen
capture, and browser automation remain outside this binding's current scope.

See the [PassoFlow repository](https://github.com/kent-tokyo/passoflow) for
the full contract and development instructions.

