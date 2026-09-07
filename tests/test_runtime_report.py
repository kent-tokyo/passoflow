import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from runtime_report import validate_runtime_report


class RuntimeReportTests(unittest.TestCase):
    def test_accepts_valid_report(self):
        report = {
            "status": "warning_continue",
            "events": [{"step": 2, "outcome": "warning_continue"}],
        }
        self.assertIs(validate_runtime_report(report), report)

    def test_rejects_missing_or_invalid_report_fields(self):
        invalid_reports = [
            None,
            {"events": []},
            {"status": "success"},
            {"status": "success", "events": [{"step": 1, "outcome": "unknown"}]},
            {"status": "success", "events": [{"step": "1", "outcome": "success"}]},
        ]
        for report in invalid_reports:
            with self.assertRaises(RuntimeError):
                validate_runtime_report(report)


if __name__ == "__main__":
    unittest.main()
