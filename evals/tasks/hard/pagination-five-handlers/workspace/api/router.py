"""Dispatches (method, path, query) to a handler and turns ApiError into an error response."""

from .errors import ApiError
from .handlers import list_comments, list_projects, list_tags, list_tasks, list_users

ROUTES = {
    "/users": list_users,
    "/projects": list_projects,
    "/tasks": list_tasks,
    "/comments": list_comments,
    "/tags": list_tags,
}


def error_body(code, message):
    """Build the JSON body of an error response."""
    return {"error": {"code": code, "message": message}}


def handle(method, path, query=None):
    """Handle one request and return (status, body). query maps parameter names to strings."""
    if method != "GET":
        return 405, error_body("method_not_allowed", f"{method} is not supported")
    handler = ROUTES.get(path)
    if handler is None:
        return 404, error_body("not_found", f"no route for {path}")
    try:
        return handler(dict(query or {}))
    except ApiError as error:
        return error.status, error_body(error.code, error.message)
