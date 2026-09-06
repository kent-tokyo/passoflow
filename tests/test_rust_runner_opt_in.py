import os
import sys
import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
RUNNER_AVAILABLE = importlib.util.find_spec("pyautogui") is not None


@unittest.skipUnless(RUNNER_AVAILABLE, "current Python runner dependencies are not installed")
class RustRunnerOptInTests(unittest.TestCase):
    def test_flag_is_scoped_to_the_root_validation_call(self):
        import rust_validator

        report = rust_validator.RustValidationReport(
            valid=False,
            errors=1,
            warnings=0,
            diagnostics=(
                {
                    "severity": "error",
                    "code": "missing_parameter",
                    "path": "steps[1]",
                    "message": "missing required parameter 'selector'",
                },
            ),
        )
        with patch.dict(os.environ, {"PASSOFLOW_USE_RUST_VALIDATOR": "1"}), patch(
            "rust_validator.validate_with_rust", return_value=report
        ) as validate:
            import run_scenario

            errors, warnings = run_scenario._validate_scenario("scenario.yaml")

        self.assertEqual(errors, ["steps[1]: missing required parameter 'selector'"])
        self.assertEqual(warnings, [])
        validate.assert_called_once()


if __name__ == "__main__":
    unittest.main()
