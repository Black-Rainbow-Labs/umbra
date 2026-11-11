import unittest

from api.router import handle


class ListHandlersTest(unittest.TestCase):
    def count(self, path, query=None):
        status, body = handle("GET", path, query)
        self.assertEqual(status, 200)
        return len(body["items"])

    def test_every_resource_lists_all_of_its_records(self):
        self.assertEqual(self.count("/users"), 25)
        self.assertEqual(self.count("/projects"), 30)
        self.assertEqual(self.count("/tasks"), 45)
        self.assertEqual(self.count("/tags"), 12)

    def test_filters(self):
        self.assertEqual(self.count("/users", {"role": "admin"}), 5)
        self.assertEqual(self.count("/projects", {"owner_id": "3"}), 4)
        self.assertEqual(self.count("/tasks", {"status": "open"}), 15)
        self.assertEqual(self.count("/tasks", {"status": "open", "project_id": "101"}), 3)
        self.assertEqual(self.count("/comments", {"task_id": "1001"}), 3)

    def test_invalid_filter_and_unknown_route(self):
        self.assertEqual(handle("GET", "/tasks", {"project_id": "x"})[0], 400)
        self.assertEqual(handle("GET", "/nothing")[0], 404)


if __name__ == "__main__":
    unittest.main()
