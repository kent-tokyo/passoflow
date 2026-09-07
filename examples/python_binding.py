"""Check PassoFlow's Rust contract and local desktop capability from Python.

Install the published package with ``python -m pip install passoflow`` or run
``maturin develop`` from ``rust/crates/passoflow-python`` while developing.
"""

from passoflow_python import contract_version, input_platform_info_json, validate_yaml


scenario = """\
title: Example
steps:
  - action: start
  - action: end
"""

print(f"contract: {contract_version()}")
print(validate_yaml(scenario))
print(input_platform_info_json())
