import io
import json
import os
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.environ["WORKSPACE"], "logsum.py")


def run_with(content, newline="\n"):
    """Write content to a temp log file and run the script on it."""
    folder = tempfile.mkdtemp()
    path = os.path.join(folder, "app.log")
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(content.replace("\n", newline))
    try:
        return subprocess.run([sys.executable, "-I", SCRIPT, path], capture_output=True, text=True, timeout=20)
    finally:
        os.remove(path)
        os.rmdir(folder)


MIXED = (
    "2024-03-05T10:15:30 ERROR api: connection refused: retry in 5s\n"
    "2024-03-05T09:00:00 INFO web: started\n"
    "\n"
    "   \n"
    "2024-03-05T11:30:10 WARN db: slow query\n"
    "garbage line\n"
    "2024-03-05T10:00:00 INFO api: ok\n"
    "2024-03-05T10:01:00 INFO api: ok again\n"
    "2024-03-05T10:02:00 WARN db: slow again\n"
    "2024-03-05T10:03:00 error api: lowercase level\n"
    "2024-03-05T10:04:00 INFO API: uppercase service\n"
    "2024-03-05T10:05:00 INFO api:nospace\n"
)

MIXED_EXPECTED = (
    "total: 6\nERROR: 1\nWARN: 2\nINFO: 3\nDEBUG: 0\nskipped: 4\n"
    "first: 2024-03-05T09:00:00\nlast: 2024-03-05T11:30:10\n"
    "top services:\n  api: 3\n  db: 2\n  web: 1\n"
)


class LogsumTests(unittest.TestCase):
    def test_mixed(self):
        result = run_with(MIXED)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, MIXED_EXPECTED)

    def test_crlf(self):
        result = run_with(MIXED, "\r\n")
        self.assertEqual(result.stdout, MIXED_EXPECTED)

    def test_no_trailing_newline(self):
        result = run_with("2024-01-01T00:00:00 DEBUG a: x")
        self.assertEqual(result.stdout, "total: 1\nERROR: 0\nWARN: 0\nINFO: 0\nDEBUG: 1\nfirst: 2024-01-01T00:00:00\nlast: 2024-01-01T00:00:00\ntop services:\n  a: 1\n")

    def test_no_skipped_line_when_clean(self):
        result = run_with("2024-01-01T00:00:00 INFO a: x\n\n\n")
        self.assertNotIn("skipped", result.stdout)

    def test_empty_file(self):
        result = run_with("")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "total: 0\n")

    def test_only_malformed(self):
        result = run_with("nope\nstill nope\n\n")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "total: 0\nskipped: 2\n")

    def test_top_three_ties_alphabetical(self):
        lines = "".join("2024-01-01T00:00:%02d INFO %s: m\n" % (i, name) for i, name in enumerate(["zed", "beta", "alpha", "beta", "zed", "gamma", "delta", "delta"]))
        result = run_with(lines)
        self.assertTrue(result.stdout.endswith("top services:\n  beta: 2\n  delta: 2\n  zed: 2\n"), result.stdout)

    def test_timestamps_out_of_order(self):
        result = run_with("2024-05-01T10:00:00 INFO a: x\n2023-12-31T23:59:59 INFO a: y\n2024-05-01T09:59:59 INFO a: z\n")
        self.assertIn("first: 2023-12-31T23:59:59\nlast: 2024-05-01T10:00:00\n", result.stdout)

    def test_service_name_characters(self):
        result = run_with("2024-01-01T00:00:00 INFO my_svc-2: x\n2024-01-01T00:00:01 INFO bad svc: x\n")
        self.assertIn("total: 1\n", result.stdout)
        self.assertIn("skipped: 1\n", result.stdout)
        self.assertTrue(result.stdout.endswith("  my_svc-2: 1\n"))

    def test_bad_timestamp_is_malformed(self):
        result = run_with("2024-01-01 00:00:00 INFO a: x\n2024-01-01T00:00:00 INFO a: ok\n")
        self.assertIn("total: 1\n", result.stdout)
        self.assertIn("skipped: 1\n", result.stdout)

    def test_missing_file(self):
        missing = os.path.join(tempfile.gettempdir(), "definitely_missing_logsum_file.log")
        result = subprocess.run([sys.executable, "-I", SCRIPT, missing], capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stderr.strip(), "error: cannot read " + missing)
        self.assertEqual(result.stdout, "")

    def test_no_argument(self):
        result = subprocess.run([sys.executable, "-I", SCRIPT], capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stderr.strip(), "usage: python logsum.py <logfile>")


def main():
    """Run all tests and print one JSON summary line."""
    if not os.path.exists(SCRIPT):
        print(json.dumps({"total": 1, "bad": ["logsum.py is missing"]}))
        return
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(LogsumTests)
    result = unittest.TextTestRunner(stream=io.StringIO()).run(suite)
    bad = ["%s: %s" % (test.id().split(".")[-1], text.strip().splitlines()[-1][:160]) for test, text in result.failures + result.errors]
    print(json.dumps({"total": result.testsRun, "bad": bad}))


main()
