"""In memory data for the API. Handlers read these lists on every call.

Every record has an id and a created_at timestamp (ISO 8601, UTC, always in the form
YYYY-MM-DDTHH:MM:SSZ). Several records share a created_at, and the lists are not in any useful order.
"""


def _permute(items, step):
    """Return the items in a fixed scrambled order (step must not share a factor with len(items))."""
    return [items[(i * step) % len(items)] for i in range(len(items))]


def _day(i, per_day):
    """Return a timestamp where per_day consecutive indexes share the same instant."""
    return f"2024-02-{1 + (i // per_day) % 28:02d}T09:00:00Z"


def _day_descending(i, per_day):
    """Return a timestamp that moves backward as the index grows, per_day indexes at a time."""
    return f"2024-02-{28 - (i // per_day) % 28:02d}T09:00:00Z"


USERS = _permute([
    {"id": 1 + i, "name": f"user{1 + i:02d}", "role": "admin" if i % 5 == 0 else "member", "created_at": _day(i, 3)}
    for i in range(25)
], 11)

PROJECTS = _permute([
    {"id": 101 + i, "owner_id": 1 + i % 7, "name": f"project{1 + i:02d}", "created_at": _day(i, 2)}
    for i in range(30)
], 11)

TASKS = _permute([
    {"id": 1001 + i, "project_id": 101 + i % 6, "status": ["open", "done", "blocked"][(i // 3) % 3], "title": f"task{1 + i:02d}", "created_at": _day(i, 4)}
    for i in range(45)
], 11)

COMMENTS = _permute([
    {"id": 5001 + i, "task_id": 1001 + i % 45, "body": f"comment{1 + i:03d}", "created_at": _day_descending(i, 5)}
    for i in range(130)
], 11)

_TAG_SLUGS = ["bug", "ui", "api", "docs", "perf", "infra", "test", "sec", "ux", "db", "ops", "ci"]
TAGS = _permute([
    {"id": slug, "label": slug.upper(), "created_at": _day(i, 2)}
    for i, slug in enumerate(_TAG_SLUGS)
], 5)
