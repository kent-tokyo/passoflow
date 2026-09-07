import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from scenario_paths import resolve_scenario_path


class ScenarioPathTests(unittest.TestCase):
    def test_resolves_nested_path_inside_root(self):
        with tempfile.TemporaryDirectory() as root:
            self.assertEqual(
                resolve_scenario_path(root, "nested/child.yaml"),
                Path(root).resolve() / "nested/child.yaml",
            )

    def test_rejects_traversal_absolute_and_empty_targets(self):
        with tempfile.TemporaryDirectory() as root:
            for target in ("../outside.yaml", "/tmp/outside.yaml", "", None):
                with self.assertRaises(ValueError):
                    resolve_scenario_path(root, target)


if __name__ == "__main__":
    unittest.main()
