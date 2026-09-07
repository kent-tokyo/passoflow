import json
import sys
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from rust_validator import (
    RustValidationReport,
    assert_plan_matches_steps,
    compare_with_python,
    normalize_with_rust,
    plan_with_rust,
    steps_from_rust_plan,
    validate_with_rust,
)

VALIDATOR = Path(__file__).resolve().parents[1] / "rust" / "target" / "debug" / "passoflow-validate"
FIXTURE = Path(__file__).parent / "fixtures" / "validator_compat" / "valid_control.yaml"
FIXTURES = FIXTURE.parent
EXPECTED = json.loads((FIXTURES / "expected_diagnostics.json").read_text(encoding="utf-8"))


class RustValidatorBridgeTests(unittest.TestCase):
    def test_bridge_is_disabled_without_explicit_binary(self):
        with patch.dict("os.environ", {}, clear=True):
            self.assertIsNone(validate_with_rust("scenario.yaml"))

    def test_report_preserves_stable_diagnostics_and_messages(self):
        payload = json.dumps(
            {
                "contract": "0.1",
                "valid": False,
                "errors": 1,
                "warnings": 0,
                "diagnostics": [
                    {
                        "severity": "error",
                        "code": "missing_parameter",
                        "path": "steps[1]",
                        "message": "missing required parameter 'selector'",
                    }
                ],
            }
        )
        with patch("rust_validator.subprocess.run") as run:
            run.return_value.stdout = payload
            run.return_value.stderr = ""
            report = validate_with_rust("scenario.yaml", "/tmp/passoflow-validate")

        self.assertIsInstance(report, RustValidationReport)
        self.assertFalse(report.valid)
        self.assertEqual(report.diagnostics[0]["code"], "missing_parameter")
        self.assertEqual(report.messages("error"), ["steps[1]: missing required parameter 'selector'"])
        run.assert_called_once()

    def test_report_rejects_unknown_contract_or_inconsistent_counts(self):
        payload = {
            "contract": "9.9",
            "valid": True,
            "errors": 0,
            "warnings": 0,
            "diagnostics": [],
        }
        with self.assertRaisesRegex(ValueError, "invalid report"):
            RustValidationReport.from_json(json.dumps(payload))

        payload["contract"] = "0.1"
        payload["valid"] = False
        with self.assertRaisesRegex(ValueError, "inconsistent counts"):
            RustValidationReport.from_json(json.dumps(payload))

        payload["valid"] = True
        payload["diagnostics"] = [{"severity": "error", "code": "bad"}]
        with self.assertRaisesRegex(ValueError, "malformed diagnostic"):
            RustValidationReport.from_json(json.dumps(payload))

    def test_bridge_converts_validator_timeout_to_actionable_error(self):
        with patch(
            "rust_validator.subprocess.run",
            side_effect=subprocess.TimeoutExpired(["passoflow-validate"], 10),
        ):
            with self.assertRaisesRegex(RuntimeError, "timed out after 10 seconds"):
                validate_with_rust("scenario.yaml", "/tmp/passoflow-validate")

    def test_plan_bridge_validates_the_versioned_shape(self):
        payload = json.dumps({"contract": "0.1", "steps": [{"index": 1, "action": "wait"}]})
        with patch("rust_validator.subprocess.run") as run:
            run.return_value.stdout = payload
            run.return_value.stderr = ""
            run.return_value.returncode = 0
            plan = plan_with_rust("scenario.yaml", "/tmp/passoflow-validate")
        self.assertEqual(plan["steps"][0]["action"], "wait")

        with patch("rust_validator.subprocess.run") as run:
            run.return_value.stdout = json.dumps({"contract": "9.9", "steps": []})
            run.return_value.stderr = ""
            run.return_value.returncode = 0
            with self.assertRaisesRegex(ValueError, "invalid execution plan"):
                plan_with_rust("scenario.yaml", "/tmp/passoflow-validate")

    def test_plan_alignment_rejects_stale_steps(self):
        plan = {"contract": "0.1", "steps": [{"index": 1, "action": "wait"}]}
        assert_plan_matches_steps(plan, [{"action": "wait"}])
        with self.assertRaisesRegex(RuntimeError, "stale"):
            assert_plan_matches_steps(plan, [{"action": "wait"}, {"action": "end"}])
        with self.assertRaisesRegex(RuntimeError, "step 1"):
            assert_plan_matches_steps(plan, [{"action": "click_image"}])

    def test_plan_alignment_rejects_stale_parameters(self):
        plan = {
            "contract": "0.1",
            "steps": [{"index": 1, "action": "browser_click", "params": {"selector": "#old"}}],
        }
        with self.assertRaisesRegex(RuntimeError, "parameters"):
            assert_plan_matches_steps(plan, [{"action": "browser_click", "selector": "#new"}])

    def test_materializes_python_steps_from_normalized_rust_parameters(self):
        plan = {
            "contract": "0.1",
            "steps": [
                {"index": 1, "action": "browser_fill", "params": {"selector": "#name", "text": "Ada"}},
                {"index": 2, "action": "wait", "params": {"ms": 5}},
            ],
        }

        self.assertEqual(
            steps_from_rust_plan(plan),
            [
                {"action": "browser_fill", "selector": "#name", "text": "Ada"},
                {"action": "wait", "ms": 5},
            ],
        )

    def test_materialized_plan_rejects_malformed_steps(self):
        with self.assertRaisesRegex(ValueError, "malformed"):
            steps_from_rust_plan({"contract": "0.1", "steps": [{"index": 2, "action": "wait"}]})

        with self.assertRaisesRegex(ValueError, "action parameter"):
            steps_from_rust_plan(
                {"contract": "0.1", "steps": [{"index": 1, "action": "wait", "params": {"action": "bad"}}]}
            )

    def test_compatibility_gate_reports_validity_mismatch(self):
        report = RustValidationReport(True, 0, 0, ())

        comparison = compare_with_python(["python rejected step"], [], report)

        self.assertFalse(comparison["valid_match"])
        self.assertFalse(comparison["ready_for_default"])
        self.assertFalse(comparison["error_count_match"])
        self.assertTrue(comparison["warning_count_match"])
        self.assertFalse(comparison["python_valid"])
        self.assertTrue(comparison["rust_valid"])
        self.assertEqual(comparison["python_errors"], ["python rejected step"])

    @unittest.skipUnless(VALIDATOR.is_file(), "Rust validator binary is not built")
    def test_bridge_reads_all_shared_fixtures_with_the_real_binary(self):
        for name, expectation in EXPECTED.items():
            report = validate_with_rust(FIXTURES / name, VALIDATOR)

            self.assertEqual(report.valid, expectation["valid"], name)
            self.assertEqual(
                [item["code"] for item in report.diagnostics],
                expectation["codes"],
                name,
            )

    @unittest.skipUnless(VALIDATOR.is_file(), "Rust validator binary is not built")
    def test_bridge_reads_deterministic_normalized_yaml(self):
        normalized = normalize_with_rust(FIXTURE, VALIDATOR)

        self.assertEqual(
            normalized,
            "title: Control flow\nsteps:\n- action: if\n  last_step: ok\n- action: else\n- action: endif\n- action: wait\n  loop: retry_block\n  loop_count: 2\n  ms: 1\n- action: wait\n  loop: retry_block\n  loop_count: 2\n  ms: 1\n",
        )

    @unittest.skipUnless(VALIDATOR.is_file(), "Rust validator binary is not built")
    def test_bridge_reads_execution_plan(self):
        plan = plan_with_rust(FIXTURE, VALIDATOR)
        self.assertEqual(plan["contract"], "0.1")
        self.assertEqual([step["index"] for step in plan["steps"]], [1, 2, 3, 4, 5])
        self.assertEqual(plan["steps"][0]["action"], "if")

    @unittest.skipUnless(VALIDATOR.is_file(), "Rust validator binary is not built")
    def test_normalized_yaml_preserves_python_yaml_values(self):
        normalized = normalize_with_rust(FIXTURE, VALIDATOR)
        original = yaml.safe_load(FIXTURE.read_text(encoding="utf-8"))

        self.assertEqual(yaml.safe_load(normalized), original)


if __name__ == "__main__":
    unittest.main()
