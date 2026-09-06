"""Use PassoFlow's Rust validator from Python.

Install the published package with ``python -m pip install passoflow`` or run
``maturin develop`` from ``rust/crates/passoflow-python`` while developing.
"""

from passoflow_python import contract_version, validate_yaml


scenario = """\
title: Example
steps:
  - action: start
  - action: end
"""

print(f"contract: {contract_version()}")
print(validate_yaml(scenario))

