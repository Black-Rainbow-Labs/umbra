import unittest

from api import reports


class ReportsTest(unittest.TestCase):
    def test_task_counts(self):
        self.assertEqual(reports.task_counts(), {"open": 15, "done": 15, "blocked": 15})
        self.assertEqual(reports.task_counts(101), {"open": 3, "done": 2, "blocked": 3})

    def test_comment_total(self):
        self.assertEqual(reports.comment_total(), 130)
        self.assertEqual(reports.comment_total(101), 23)

    def test_admin_names(self):
        self.assertEqual(reports.admin_names(), ["user01", "user06", "user11", "user16", "user21"])

    def test_project_names_by_owner(self):
        names = reports.project_names_by_owner()
        self.assertEqual(sorted(names), [1, 2, 3, 4, 5, 6, 7])
        self.assertEqual(names[1], ["project01", "project08", "project15", "project22", "project29"])
        self.assertEqual(sum(len(v) for v in names.values()), 30)


if __name__ == "__main__":
    unittest.main()
