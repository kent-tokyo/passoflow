import ast
import json
import os
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PYTHON_RUNNER = ROOT / "src" / "run_scenario.py"
VALIDATOR = Path(os.environ.get("PASSOFLOW_VALIDATE_BIN", ROOT / "rust" / "target" / "debug" / "passoflow-validate"))


def _assignment_nodes(module: ast.Module) -> dict[str, ast.expr]:
    assignments = {}
    for statement in module.body:
        if isinstance(statement, ast.Assign):
            for target in statement.targets:
                if isinstance(target, ast.Name):
                    assignments[target.id] = statement.value
    return assignments


def _evaluate(node: ast.expr, assignments: dict[str, ast.expr]):
    if isinstance(node, ast.Name):
        return _evaluate(assignments[node.id], assignments)
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.BitOr):
        return _evaluate(node.left, assignments) | _evaluate(node.right, assignments)
    if isinstance(node, ast.Dict):
        return {_evaluate(key, assignments): _evaluate(value, assignments) for key, value in zip(node.keys, node.values, strict=True)}
    if isinstance(node, ast.List):
        return [_evaluate(item, assignments) for item in node.elts]
    if isinstance(node, ast.Tuple):
        return tuple(_evaluate(item, assignments) for item in node.elts)
    if isinstance(node, ast.Set):
        return {_evaluate(item, assignments) for item in node.elts}
    return ast.literal_eval(node)


@unittest.skipUnless(VALIDATOR.is_file(), "Rust validator binary is not built")
class ActionSchemaContractTests(unittest.TestCase):
    def test_python_validator_schema_matches_rust_cli_schema(self):
        module = ast.parse(PYTHON_RUNNER.read_text(encoding="utf-8"))
        assignments = _assignment_nodes(module)
        python_schema = _evaluate(assignments["_ACTION_SCHEMA"], assignments)
        python_meta_keys = _evaluate(assignments["_STEP_META_KEYS"], assignments)
        result = subprocess.run([str(VALIDATOR), "--schema"], check=True, capture_output=True, text=True)
        rust_schema = json.loads(result.stdout)

        self.assertEqual(rust_schema["contract"], "0.1")
        self.assertEqual(sorted(rust_schema["step_meta_keys"]), sorted(python_meta_keys))
        expected_actions = [
            {
                "name": action,
                "required": sorted(required),
                "optional": sorted(optional),
            }
            for action, (required, optional) in sorted(python_schema.items())
        ]
        actual_actions = [
            {
                "name": action["name"],
                "required": sorted(action["required"]),
                "optional": sorted(action["optional"]),
            }
            for action in rust_schema["actions"]
        ]
        self.assertEqual(sorted(actual_actions, key=lambda action: action["name"]), expected_actions)


if __name__ == "__main__":
    unittest.main()
