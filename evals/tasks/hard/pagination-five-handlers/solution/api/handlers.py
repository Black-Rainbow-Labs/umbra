"""The five list handlers. Each takes the query dict (string values) and returns (status, body)."""

from . import store
from .errors import ApiError
from .pagination import paginate


def _int_filter(query, name):
    """Return the integer value of an optional filter parameter, or None when it is absent."""
    if name not in query:
        return None
    if not query[name].isdigit():
        raise ApiError(400, "invalid_filter", f"{name} must be an integer")
    return int(query[name])


def list_users(query):
    """List users, optionally only those with ?role=admin or ?role=member, one page at a time."""
    filters = {"role": query["role"]} if "role" in query else {}
    items = [dict(u) for u in store.USERS if "role" not in query or u["role"] == query["role"]]
    return paginate("users", items, query, filters)


def list_projects(query):
    """List projects, optionally only those with ?owner_id=<user id>, one page at a time."""
    owner_id = _int_filter(query, "owner_id")
    filters = {} if owner_id is None else {"owner_id": owner_id}
    items = [dict(p) for p in store.PROJECTS if owner_id is None or p["owner_id"] == owner_id]
    return paginate("projects", items, query, filters)


def list_tasks(query):
    """List tasks, optionally filtered by ?status and ?project_id, one page at a time."""
    project_id = _int_filter(query, "project_id")
    filters = {}
    if "status" in query:
        filters["status"] = query["status"]
    if project_id is not None:
        filters["project_id"] = project_id
    items = [
        dict(t) for t in store.TASKS
        if ("status" not in query or t["status"] == query["status"]) and (project_id is None or t["project_id"] == project_id)
    ]
    return paginate("tasks", items, query, filters)


def list_comments(query):
    """List comments, optionally only those of ?task_id=<task id>, one page at a time."""
    task_id = _int_filter(query, "task_id")
    filters = {} if task_id is None else {"task_id": task_id}
    items = [dict(c) for c in store.COMMENTS if task_id is None or c["task_id"] == task_id]
    return paginate("comments", items, query, filters)


def list_tags(query):
    """List all tags, one page at a time."""
    return paginate("tags", [dict(t) for t in store.TAGS], query, {})
