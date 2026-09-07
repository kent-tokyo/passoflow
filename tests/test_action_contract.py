import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from action_contract import action_outcome_contract


class ActionContractTests(unittest.TestCase):
    def test_recoverable_and_fatal_warning_policies(self):
        self.assertTrue(action_outcome_contract("click_image")["warning_continue"])
        self.assertFalse(action_outcome_contract("browser_click")["warning_continue"])
        self.assertTrue(action_outcome_contract("send_webhook")["warning_continue"])
        self.assertFalse(
            action_outcome_contract("send_webhook", {"on_error": "stop"})["warning_continue"]
        )

    def test_every_contract_always_stops_on_unexpected_failure(self):
        for action in ("click_image", "browser_click", "send_webhook"):
            self.assertTrue(action_outcome_contract(action)["success"])
            self.assertTrue(action_outcome_contract(action)["failure_stop"])


if __name__ == "__main__":
    unittest.main()
