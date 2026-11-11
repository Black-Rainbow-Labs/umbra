"""Runs test_hidden.py and prints {"ClassName": {"passed": n, "failed": n}} as JSON. Run with the workspace as cwd."""

import importlib.util
import json
import pathlib
import sys
import unittest


class Collector(unittest.TestResult):
    """Counts passed and failed tests per test class."""

    def __init__(self):
        super().__init__()
        self.counts = {}

    def _bump(self, test, key):
        name = type(test).__name__
        self.counts.setdefault(name, {"passed": 0, "failed": 0})[key] += 1

    def addSuccess(self, test):
        self._bump(test, "passed")

    def addFailure(self, test, err):
        self._bump(test, "failed")

    def addError(self, test, err):
        self._bump(test, "failed")


def main():
    path = pathlib.Path(__file__).with_name("test_hidden.py")
    spec = importlib.util.spec_from_file_location("test_hidden", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["test_hidden"] = module
    spec.loader.exec_module(module)
    result = Collector()
    unittest.defaultTestLoader.loadTestsFromModule(module).run(result)
    print(json.dumps(result.counts))


main()
