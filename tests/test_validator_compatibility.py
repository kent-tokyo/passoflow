import sys
import unittest
import importlib.util
import json
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

RUN_SCENARIO_AVAILABLE = importlib.util.find_spec("pyautogui") is not None
if RUN_SCENARIO_AVAILABLE:
    import run_scenario


FIXTURES = Path(__file__).parent / "fixtures" / "validator_compat"
EXPECTED = json.loads((FIXTURES / "expected_diagnostics.json").read_text(encoding="utf-8"))


@unittest.skipUnless(RUN_SCENARIO_AVAILABLE, "current Python runner dependencies are not installed")
class ValidatorCompatibilityTests(unittest.TestCase):
    def validate(self, name):
        return run_scenario._validate_scenario(FIXTURES / name)

    def test_shared_fixtures_match_rust_validity_contract(self):
        for name, expectation in EXPECTED.items():
            errors, _ = self.validate(name)
            self.assertEqual(not errors, expectation["valid"], name)

    def test_shared_fixtures_keep_expected_python_messages(self):
        errors, _ = self.validate("unknown_action.yaml")
        self.assertIn("unknown action", errors[0])
        errors, _ = self.validate("missing_parameter.yaml")
        self.assertIn("missing required parameter", errors[0])


if __name__ == "__main__":
    unittest.main()
