"""Summaries built on the list handlers. They need every matching record, not one page."""

from .handlers import list_comments, list_projects, list_tasks, list_users
from .pagination import collect_all


def task_counts(project_id=None):
    """Return {status: number of tasks} for one project, or for every task when project_id is None."""
    query = {} if project_id is None else {"project_id": str(project_id)}
    counts = {}
    for task in collect_all(list_tasks, query):
        counts[task["status"]] = counts.get(task["status"], 0) + 1
    return counts


def comment_total(project_id=None):
    """Return how many comments belong to the tasks of one project, or to all tasks when project_id is None."""
    if project_id is None:
        return len(collect_all(list_comments, {}))
    total = 0
    for task in collect_all(list_tasks, {"project_id": str(project_id)}):
        total += len(collect_all(list_comments, {"task_id": str(task["id"])}))
    return total


def admin_names():
    """Return the sorted names of all users with the admin role."""
    return sorted(user["name"] for user in collect_all(list_users, {"role": "admin"}))


def project_names_by_owner():
    """Return {owner_id: sorted project names} for every project."""
    names = {}
    for project in collect_all(list_projects, {}):
        names.setdefault(project["owner_id"], []).append(project["name"])
    return {owner_id: sorted(values) for owner_id, values in names.items()}
