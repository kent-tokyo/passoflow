import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from rust_action_coverage import unsupported_actions


class RustActionCoverageTests(unittest.TestCase):
    def test_ignores_structural_actions_and_returns_sorted_missing_adapters(self):
        steps = [
            {"action": "endif"},
            {"action": "browser_click"},
            {"action": "if"},
            {"action": "unknown_z"},
            {"action": "unknown_a"},
        ]
        self.assertEqual(unsupported_actions(steps, {"browser_click"}), ["unknown_a", "unknown_z"])

    def test_malformed_step_is_reported_before_dispatch(self):
        self.assertEqual(unsupported_actions([{}], {"wait"}), ["<missing action>"])


if __name__ == "__main__":
    unittest.main()
