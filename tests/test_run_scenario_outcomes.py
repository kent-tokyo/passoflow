import sys
import unittest
import importlib.util
import tempfile
from types import SimpleNamespace
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

RUNNER_AVAILABLE = importlib.util.find_spec("pyautogui") is not None
if RUNNER_AVAILABLE:
    import run_scenario


@unittest.skipUnless(RUNNER_AVAILABLE, "current Python runner dependencies are not installed")
class ImageSearchSafetyTests(unittest.TestCase):
    def test_active_window_region_is_offset_from_foreground_window(self):
        import screen_actions

        window = SimpleNamespace(left=100, top=200, width=800, height=600)
        with patch.object(screen_actions.gw, "getActiveWindow", return_value=window):
            self.assertEqual(
                screen_actions.resolve_search_region((10, 20, 300, 200), "active_window"),
                (110, 220, 300, 200),
            )
            self.assertEqual(
                screen_actions.resolve_search_region(None, "active_window"),
                (100, 200, 800, 600),
            )

    def test_target_window_guard_fails_closed_on_title_mismatch(self):
        import screen_actions

        window = SimpleNamespace(title="Other Window")
        with patch.object(screen_actions.gw, "getActiveWindow", return_value=window):
            with self.assertRaisesRegex(RuntimeError, "Target window check failed"):
                screen_actions.ensure_target_window("Expected Window")


@unittest.skipUnless(RUNNER_AVAILABLE, "current Python runner dependencies are not installed")
class LastStepConditionTests(unittest.TestCase):
    def test_action_outcome_contract_marks_recoverable_warnings(self):
        self.assertEqual(
            run_scenario.action_outcome_contract("click_image"),
            {"success": True, "warning_continue": True, "failure_stop": True},
        )
        self.assertEqual(
            run_scenario.action_outcome_contract("send_webhook", {"on_error": "stop"})["warning_continue"],
            False,
        )
        self.assertTrue(run_scenario.action_outcome_contract("browser_click")["failure_stop"])

    def test_rust_warning_outcome_follows_action_contract(self):
        self.assertEqual(run_scenario._rust_warning_outcome("click_image", {}, True), "warning_continue")
        self.assertEqual(run_scenario._rust_warning_outcome("browser_click", {}, True), "failure_stop")
        self.assertEqual(
            run_scenario._rust_warning_outcome("send_webhook", {"on_error": "continue"}, True),
            "warning_continue",
        )
        self.assertEqual(
            run_scenario._rust_warning_outcome("send_webhook", {"on_error": "stop"}, True),
            "failure_stop",
        )
        self.assertEqual(run_scenario._rust_warning_outcome("browser_click", {}, False), "success")

    def test_last_step_condition_matches_warning_status(self):
        state = {"last_step": "warned"}

        self.assertTrue(run_scenario._evaluate_condition({"last_step": "warned"}, {}, state))
        self.assertFalse(run_scenario._evaluate_condition({"last_step": "ok"}, {}, state))

    def test_validator_accepts_last_step_instead_of_variable(self):
        errors = []
        warnings = []

        run_scenario._validate_step(
            {"action": "if", "last_step": "warned"}, "test step", errors, warnings
        )

        self.assertEqual(errors, [])

    def test_warning_outcome_can_select_the_true_branch(self):
        def warning_action(_step, _variables):
            run_scenario.logger.warning("synthetic warning")

        steps = [
            {"action": "warn"},
            {"action": "if", "last_step": "warned"},
            {"action": "set_variable", "name": "branch", "value": "warning"},
            {"action": "else"},
            {"action": "set_variable", "name": "branch", "value": "ok"},
            {"action": "endif"},
        ]
        variables = {}

        with patch.dict(run_scenario.ACTIONS, {"warn": warning_action}), patch.object(run_scenario.time, "sleep"):
            run_scenario._run_steps(steps, variables)

        self.assertEqual(variables["branch"], "warning")

    def test_webhook_failure_can_stop_or_continue(self):
        error = __import__("urllib.error", fromlist=["URLError"]).URLError("offline")
        with patch.object(run_scenario.urllib.request, "urlopen", side_effect=error):
            run_scenario._run_send_webhook({"url": "https://example.invalid", "on_error": "continue"}, {})
            with self.assertRaises(RuntimeError):
                run_scenario._run_send_webhook({"url": "https://example.invalid", "on_error": "stop"}, {})

    def test_webhook_on_error_policy_is_validated(self):
        errors = []
        run_scenario._validate_step(
            {"action": "send_webhook", "url": "https://example.invalid", "on_error": "retry"},
            "test step",
            errors,
            [],
        )
        self.assertIn("on_error", errors[0])

    def test_completed_action_emits_machine_readable_completion_marker(self):
        with patch.dict(run_scenario.ACTIONS, {"noop": lambda _step, _variables: None}), \
             patch.object(run_scenario, "print") as print_mock, \
             patch.object(run_scenario.time, "sleep"):
            run_scenario._run_steps([{"action": "noop"}], {})

        self.assertIn("@@COMPLETED@@1/1", [call.args[0] for call in print_mock.call_args_list])

    def test_run_outcome_classification_distinguishes_warning_stop_and_failure(self):
        import api_server

        self.assertEqual(api_server._classify_run_outcome(0, False, []), "success")
        self.assertEqual(api_server._classify_run_outcome(0, False, ["[WARNING] skipped"]), "warning")
        self.assertEqual(api_server._classify_run_outcome(1, True, []), "stopped")
        self.assertEqual(api_server._classify_run_outcome(1, False, []), "failed")

    def test_every_action_has_localized_palette_purpose(self):
        import api_server

        actions = {schema["action"] for schema in api_server.ACTION_SCHEMA}
        self.assertEqual(actions, set(api_server.ACTION_PURPOSES))
        for purpose in api_server.ACTION_PURPOSES.values():
            self.assertTrue(all(purpose[locale].strip() for locale in ("en", "ja", "zh")))

    def test_file_actions_are_in_file_operations_category(self):
        import api_server

        categories = {schema["action"]: schema["category"] for schema in api_server.ACTION_SCHEMA}
        self.assertEqual({categories[action] for action in ("rename_file", "move_file", "copy_file")}, {"file"})
        self.assertEqual(categories["launch_app"], "app")

    def test_launch_app_startup_options_are_validated(self):
        errors = []
        warnings = []
        run_scenario._validate_step(
            {
                "action": "launch_app",
                "path": "app.exe",
                "wait_for_window": "Ready",
                "startup_timeout_ms": 5000,
            },
            "test step",
            errors,
            warnings,
        )
        self.assertEqual(errors, [])

        errors = []
        run_scenario._validate_step(
            {"action": "launch_app", "path": "app.exe", "startup_timeout_ms": -1},
            "test step",
            errors,
            [],
        )
        self.assertIn("startup_timeout_ms", errors[0])

    def test_local_vite_fallback_origin_is_allowed_by_cors(self):
        from fastapi.testclient import TestClient
        import api_server

        with TestClient(api_server.app) as client:
            response = client.options(
                "/api/actions",
                headers={
                    "Origin": "http://localhost:4174",
                    "Access-Control-Request-Method": "GET",
                },
            )

        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.headers.get("access-control-allow-origin"), "http://localhost:4174")

    def test_reserved_images_folder_is_rejected_with_windows_separator(self):
        import api_server

        with self.assertRaises(api_server.HTTPException):
            api_server._reject_reserved_path(r"images\scenario\capture.png")

        with self.assertRaises(api_server.HTTPException):
            api_server._reject_reserved_path("images/scenario/capture.png")

    def test_csv_import_accepts_cp932_japanese_csv(self):
        import input_actions

        with tempfile.TemporaryDirectory() as temp_dir:
            csv_path = Path(temp_dir) / "japanese.csv"
            csv_path.write_bytes('"列名","金額"\n"入荷番号","123"\n'.encode("cp932"))

            rows = input_actions._read_csv_rows(str(csv_path))

        self.assertEqual(rows, [{"列名": "入荷番号", "金額": "123"}])

    def test_table_loop_skips_unchecked_rows(self):
        rows = [{"code": "A"}, {"code": "B"}, {"code": "C"}]
        variables = {}
        steps = [
            {"action": "load_table", "path": "rows.csv", "name": "rows", "selected_rows": [0, 2]},
            {"action": "concat_variable", "name": "seen", "value": "{{code}}", "loop": "rows", "loop_table": "rows"},
        ]

        with patch.object(run_scenario, "read_table_rows", return_value=rows), patch.object(run_scenario.time, "sleep"):
            run_scenario._LOADED_TABLES.clear()
            run_scenario._run_steps(steps, variables)

        self.assertEqual(variables["seen"], "C")
        self.assertEqual(run_scenario._LOADED_TABLES["rows"], [{"code": "A"}, {"code": "C"}])

    def test_static_preflight_finds_undefined_variables_and_tables(self):
        self.assertEqual(
            run_scenario._find_static_reference_warnings(
                [{"action": "paste_variable", "name": "missing"}], "demo.yaml"
            ),
            ["demo.yaml step 1 (paste_variable): variable not defined in this scenario: missing"],
        )
        self.assertEqual(
            run_scenario._find_static_reference_warnings(
                [{"action": "loop", "loop": "rows", "loop_table": "missing"}], "demo.yaml"
            ),
            ["demo.yaml step 1: loop table not loaded in this scenario: missing"],
        )

    def test_local_run_feedback_suggests_fixes_without_ai(self):
        import api_server

        feedback = api_server._build_local_run_feedback(
            [
                "[WARNING] None of the images were found on screen after 1 attempt(s): ['button.png']",
                "[WARNING] No window found with title containing after 1 attempt(s): Notepad",
            ]
        )

        self.assertIn("retry", feedback)
        self.assertIn("wait", feedback)
        self.assertIn("confidence", feedback)

    def test_image_search_validation_rejects_unsafe_tuning_values(self):
        errors = []
        warnings = []
        run_scenario._validate_step(
            {
                "action": "click_image",
                "images": [],
                "confidence": 1.2,
                "retry": -1,
                "retry_interval_ms": -10,
                "click_indicator_duration": -0.1,
            },
            "test step",
            errors,
            warnings,
        )
        self.assertTrue(any("confidence" in error for error in errors))
        self.assertTrue(any("retry" in error for error in errors))
        self.assertTrue(any("retry_interval_ms" in error for error in errors))
        self.assertTrue(any("click_indicator_duration" in error for error in errors))

    def test_image_search_validation_rejects_invalid_region(self):
        errors = []
        warnings = []
        run_scenario._validate_step(
            {"action": "click_image", "images": [], "region": [0, 0, 0, 200]},
            "test step", errors, warnings,
        )
        self.assertTrue(any("region" in error for error in errors))

    def test_browser_action_validation_rejects_invalid_wait_options(self):
        errors = []
        warnings = []
        run_scenario._validate_step(
            {"action": "browser_wait_for", "selector": "#ready", "state": "loaded", "timeout_ms": 0},
            "test step", errors, warnings,
        )
        self.assertTrue(any("state" in error for error in errors))
        self.assertTrue(any("timeout_ms" in error for error in errors))

    def test_false_last_step_condition_without_else_does_not_crash(self):
        steps = [
            {"action": "if", "last_step": "warned"},
            {"action": "set_variable", "name": "branch", "value": "unexpected"},
            {"action": "endif"},
        ]

        with patch.object(run_scenario.time, "sleep"):
            run_scenario._run_steps(steps, {})

    def test_preflight_response_reports_validation_errors_and_warnings(self):
        import api_server

        with patch.object(api_server, "_resolve_scenario_path", return_value=Path("scenario.yaml")), \
             patch.object(Path, "exists", return_value=True), \
             patch.object(api_server, "_validate_scenario", return_value=(["bad step"], ["check image"])):
            response = api_server.validate_scenario("scenario.yaml")

        self.assertEqual(response, {"errors": ["bad step"], "warnings": ["check image"]})

    def test_scenario_path_accepts_normal_windows_file_representations(self):
        import api_server

        with tempfile.TemporaryDirectory() as temp_dir:
            scenarios_dir = Path(temp_dir)
            expected = (scenarios_dir / "folder" / "scenario with spaces.yaml").resolve()
            absolute_windows = str(expected).replace("/", "\\")
            with patch.object(api_server, "SCENARIOS_DIR", scenarios_dir):
                for filename in (
                    "folder/scenario with spaces.yaml",
                    "folder\\scenario with spaces.yaml",
                    f"scenarios/folder/scenario with spaces.yaml",
                    f"scenarios\\folder\\scenario with spaces.yaml",
                    absolute_windows,
                ):
                    self.assertEqual(api_server._resolve_scenario_path(filename), expected)


if __name__ == "__main__":
    unittest.main()
