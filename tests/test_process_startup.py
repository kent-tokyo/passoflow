import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from process_startup import wait_for_window


class FakeProcess:
    def __init__(self, polls):
        self.polls = iter(polls)
        self.returncode = 1

    def poll(self):
        value = next(self.polls)
        if value is not None:
            self.returncode = value
        return value


class ProcessStartupTests(unittest.TestCase):
    def test_waits_until_window_is_ready(self):
        clock = iter([0.0, 0.0, 0.1, 0.1])
        windows = iter([[], [object()]])
        wait_for_window(FakeProcess([None, None]), "Demo", 500, lambda _: next(windows), lambda _: None, lambda: next(clock))

    def test_rejects_early_exit_and_timeout(self):
        with self.assertRaisesRegex(RuntimeError, "exited"):
            wait_for_window(FakeProcess([7]), "Demo", 500, lambda _: [], lambda _: None, lambda: 0.0)
        with self.assertRaises(TimeoutError):
            wait_for_window(
                FakeProcess([None]), "Demo", 1000, lambda _: [], lambda _: None, iter([0.0, 1.1]).__next__
            )


if __name__ == "__main__":
    unittest.main()
