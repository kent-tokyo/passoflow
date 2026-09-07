"""Python interface to PassoFlow's Rust scenario contract."""

from . import passoflow_python as _native

contract_version = _native.contract_version
action_schema_json = _native.action_schema_json
normalize_yaml = _native.normalize_yaml
validate_yaml = _native.validate_yaml
DomBrowser = _native.DomBrowser
__version__ = _native.__version__

__all__ = ["DomBrowser", "action_schema_json", "contract_version", "normalize_yaml", "validate_yaml"]
