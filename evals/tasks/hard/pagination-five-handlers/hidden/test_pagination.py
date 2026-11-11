import base64
import glob
import os
import re
import sys
import unittest

WORKSPACE = os.environ["WORKSPACE_DIR"]
sys.path.insert(0, WORKSPACE)

from api import store  # noqa: E402
from api.router import handle  # noqa: E402

LISTS = {"/users": "USERS", "/projects": "PROJECTS", "/tasks": "TASKS", "/comments": "COMMENTS", "/tags": "TAGS"}
INT_FILTERS = {"owner_id", "project_id", "task_id"}
CURSOR_CHARS = re.compile(r"^[A-Za-z0-9_-]+$")


def b64url(text):
    return base64.urlsafe_b64encode(text.encode()).decode().rstrip("=")


def key_of(item):
    return (item["created_at"], item["id"])


def expected(path, params=None):
    """The correct full ordered list for a path and filter params, computed from the live store."""
    params = params or {}
    result = []
    for item in getattr(store, LISTS[path]):
        ok = True
        for name, value in params.items():
            if name in ("limit", "cursor"):
                continue
            ok = ok and (item[name] == int(value) if name in INT_FILTERS else item[name] == value)
        if ok:
            result.append(dict(item))
    return sorted(result, key=key_of)


def get(path, **params):
    return handle("GET", path, {k: str(v) for k, v in params.items()})


def walk(path, limit, **params):
    """Follow next_cursor to the end. Returns (items, pages)."""
    items, pages, cursor = [], 0, None
    while True:
        query = dict(params, limit=limit)
        if cursor is not None:
            query["cursor"] = cursor
        status, body = get(path, **query)
        assert status == 200, (path, status, body)
        pages += 1
        items.extend(body["items"])
        cursor = body["next_cursor"]
        if cursor is None:
            return items, pages
        assert pages < 500


class PaginationTest(unittest.TestCase):
    def setUp(self):
        self.snapshot = {name: list(getattr(store, name)) for name in LISTS.values()}

    def tearDown(self):
        for name, items in self.snapshot.items():
            getattr(store, name)[:] = items

    def test_default_page_size_and_response_shape(self):
        for path in LISTS:
            status, body = get(path)
            full = expected(path)
            self.assertEqual(status, 200, path)
            self.assertEqual(set(body), {"items", "next_cursor"}, path)
            self.assertEqual(body["items"], full[:20], path)
            if len(full) > 20:
                self.assertIsInstance(body["next_cursor"], str, path)
                self.assertRegex(body["next_cursor"], CURSOR_CHARS, path)
            else:
                self.assertIsNone(body["next_cursor"], path)

    def test_limit_is_clamped_to_one_hundred(self):
        full = expected("/comments")
        self.assertEqual(get("/comments", limit=100)[1]["items"], full[:100])
        self.assertEqual(get("/comments", limit=500)[1]["items"], full[:100])
        self.assertEqual(get("/comments", limit=99999999999999999999)[1]["items"], full[:100])
        self.assertIsNotNone(get("/comments", limit=500)[1]["next_cursor"])
        self.assertEqual(get("/comments", limit=1)[1]["items"], full[:1])
        self.assertEqual(get("/comments", limit="007")[1]["items"], full[:7])

    def test_invalid_limits_are_rejected_on_every_resource(self):
        for path in LISTS:
            for bad in ["0", "-1", "abc", "1.5", "", "+5", " 5", "5 ", "1_0", "٣", "0x10", "00"]:
                status, body = get(path, limit=bad)
                self.assertEqual(status, 400, (path, bad))
                self.assertEqual(body["error"]["code"], "invalid_limit", (path, bad))
                self.assertIsInstance(body["error"]["message"], str)
                self.assertEqual(set(body), {"error"})

    def test_walking_every_resource_gives_the_sorted_list_without_gaps_or_repeats(self):
        for path in LISTS:
            full = expected(path)
            for limit in (7, 20, 1 if path == "/tags" else 13):
                items, pages = walk(path, limit)
                self.assertEqual(items, full, (path, limit))
                self.assertEqual(pages, -(-len(full) // limit), (path, limit))

    def test_page_boundaries_have_no_empty_last_page(self):
        status, body = get("/users", limit=25)
        self.assertEqual((len(body["items"]), body["next_cursor"]), (25, None))
        status, body = get("/users", limit=24)
        self.assertEqual(len(body["items"]), 24)
        status, body = get("/users", limit=24, cursor=body["next_cursor"])
        self.assertEqual((status, len(body["items"]), body["next_cursor"]), (200, 1, None))
        status, body = get("/tags", limit=6)
        status, body = get("/tags", limit=6, cursor=body["next_cursor"])
        self.assertEqual((len(body["items"]), body["next_cursor"]), (6, None))

    def test_ordering_is_created_at_then_id_with_numeric_and_string_ids(self):
        for path in LISTS:
            items = get(path, limit=100)[1]["items"]
            keys = [key_of(i) for i in items]
            self.assertEqual(keys, sorted(keys), path)
        ids = [i["id"] for i in get("/tags")[1]["items"] if i["created_at"] == get("/tags")[1]["items"][0]["created_at"]]
        self.assertEqual(ids, sorted(ids))
        comments = get("/comments", limit=10)[1]["items"]
        self.assertNotEqual([c["id"] for c in comments], sorted(c["id"] for c in comments)[:10])

    def test_filters_combine_with_pagination(self):
        cases = [
            ("/users", {"role": "admin"}, 2),
            ("/projects", {"owner_id": "3"}, 3),
            ("/tasks", {"status": "open"}, 4),
            ("/tasks", {"status": "open", "project_id": "101"}, 2),
            ("/comments", {"task_id": "1001"}, 2),
        ]
        for path, params, limit in cases:
            items, pages = walk(path, limit, **params)
            self.assertEqual(items, expected(path, params), (path, params))
            self.assertGreater(len(items), 0)
            self.assertGreater(pages, 1, (path, params))
        status, body = get("/tasks", status="nothing")
        self.assertEqual((status, body), (200, {"items": [], "next_cursor": None}))

    def test_bad_cursors_are_rejected_on_every_resource(self):
        bad_cursors = ["", "!!!", "abc", "A" * 40, "====", "x y", b64url("hello"), b64url('{"a":1}'), b64url("[]"), b64url("null"), b64url('{"r":1,"f":2,"k":3}'), "é"]
        for path in LISTS:
            for bad in bad_cursors:
                status, body = get(path, cursor=bad)
                self.assertEqual(status, 400, (path, bad))
                self.assertEqual(body["error"]["code"], "invalid_cursor", (path, bad))

    def test_limit_is_checked_before_the_cursor(self):
        status, body = get("/users", limit=0, cursor="!!!")
        self.assertEqual((status, body["error"]["code"]), (400, "invalid_limit"))

    def test_cursor_only_works_on_the_resource_that_issued_it(self):
        for source in ("/users", "/projects", "/tasks", "/comments"):
            cursor = get(source, limit=5)[1]["next_cursor"]
            self.assertEqual(get(source, limit=5, cursor=cursor)[0], 200, source)
            for target in LISTS:
                if target != source:
                    status, body = get(target, limit=5, cursor=cursor)
                    self.assertEqual((status, body["error"]["code"]), (400, "invalid_cursor"), (source, target))

    def test_cursor_only_works_with_the_filters_it_was_issued_for(self):
        cursor = get("/tasks", status="open", limit=3)[1]["next_cursor"]
        self.assertEqual(get("/tasks", status="open", limit=3, cursor=cursor)[0], 200)
        for params in ({"status": "done"}, {}, {"status": "open", "project_id": 101}):
            status, body = get("/tasks", limit=3, cursor=cursor, **params)
            self.assertEqual((status, body["error"]["code"]), (400, "invalid_cursor"), params)
        unfiltered = get("/tasks", limit=3)[1]["next_cursor"]
        self.assertEqual(get("/tasks", limit=3, cursor=unfiltered, status="open")[1]["error"]["code"], "invalid_cursor")
        admin = get("/users", role="admin", limit=2)[1]["next_cursor"]
        self.assertEqual(get("/users", limit=2, cursor=admin)[1]["error"]["code"], "invalid_cursor")

    def test_cursor_survives_inserts_and_deletes_between_pages(self):
        status, first = get("/tasks", limit=5)
        last = first["items"][-1]
        store.TASKS.append({"id": 9001, "project_id": 101, "status": "open", "title": "early", "created_at": "2024-01-01T00:00:00Z"})
        store.TASKS.append({"id": 9002, "project_id": 101, "status": "open", "title": "late", "created_at": "2024-12-01T00:00:00Z"})
        store.TASKS.append({"id": last["id"] + 5000, "project_id": 101, "status": "open", "title": "tie after", "created_at": last["created_at"]})
        store.TASKS.append({"id": 0, "project_id": 101, "status": "open", "title": "tie before", "created_at": last["created_at"]})
        store.TASKS[:] = [t for t in store.TASKS if t["id"] != last["id"]]
        status, second = get("/tasks", limit=5, cursor=first["next_cursor"])
        self.assertEqual(status, 200)
        after = key_of(last)
        want = [t for t in expected("/tasks") if key_of(t) > after][:5]
        self.assertEqual(second["items"], want)
        shown = {t["id"] for t in first["items"]}
        self.assertFalse(shown & {t["id"] for t in second["items"]})
        rest, cursor = list(second["items"]), second["next_cursor"]
        while cursor is not None:
            status, page = get("/tasks", limit=5, cursor=cursor)
            self.assertEqual(status, 200)
            rest.extend(page["items"])
            cursor = page["next_cursor"]
        rest_ids = [t["id"] for t in rest]
        self.assertEqual(rest, [t for t in expected("/tasks") if key_of(t) > after])
        self.assertIn(last["id"] + 5000, rest_ids)
        self.assertIn(9002, rest_ids)
        self.assertNotIn(9001, rest_ids)
        self.assertNotIn(0, rest_ids)

    def test_the_same_request_gives_the_same_cursor_and_pages_have_different_cursors(self):
        a = get("/comments", limit=10)[1]["next_cursor"]
        b = get("/comments", limit=10)[1]["next_cursor"]
        self.assertEqual(a, b)
        c = get("/comments", limit=10, cursor=a)[1]["next_cursor"]
        self.assertNotEqual(a, c)

    def test_other_behavior_is_unchanged(self):
        self.assertEqual(handle("GET", "/nothing", {})[0], 404)
        self.assertEqual(handle("POST", "/users", {})[0], 405)
        status, body = get("/tasks", project_id="x")
        self.assertEqual((status, body["error"]["code"]), (400, "invalid_filter"))
        self.assertEqual(get("/tags", whatever="x")[0], 200)
        self.assertEqual(handle("GET", "/tags")[0], 200)
        item = get("/users", limit=1)[1]["items"][0]
        self.assertEqual(set(item), {"id", "name", "role", "created_at"})

    def test_pagination_logic_lives_in_one_module(self):
        sources = {}
        for path in glob.glob(os.path.join(WORKSPACE, "api", "**", "*.py"), recursive=True):
            with open(path, encoding="utf-8") as handle_file:
                sources[os.path.basename(path)] = handle_file.read()
        encoders = [name for name, text in sources.items() if "base64" in text or "b64" in text]
        limit_errors = [name for name, text in sources.items() if "invalid_limit" in text]
        cursor_errors = [name for name, text in sources.items() if "invalid_cursor" in text]
        self.assertEqual(len(encoders), 1, encoders)
        self.assertEqual(len(limit_errors), 1, limit_errors)
        self.assertEqual(len(cursor_errors), 1, cursor_errors)
        handlers = sources["handlers.py"]
        self.assertNotIn("base64", handlers)
        self.assertNotIn("invalid_limit", handlers)


if __name__ == "__main__":
    unittest.main(verbosity=2)
