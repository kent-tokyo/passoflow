"""Python interface to PassoFlow's Rust scenario contract."""

from . import passoflow_python as _native

contract_version = _native.contract_version
action_schema_json = _native.action_schema_json
input_platform_info_json = _native.input_platform_info_json
expand_nested_steps = _native.expand_nested_steps
normalize_yaml = _native.normalize_yaml
validate_yaml = _native.validate_yaml
DomBrowser = _native.DomBrowser
__version__ = _native.__version__

__all__ = ["DomBrowser", "action_schema_json", "contract_version", "expand_nested_steps", "input_platform_info_json", "normalize_yaml", "validate_yaml"]
