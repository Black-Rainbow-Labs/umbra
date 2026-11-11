"""Cursor pagination shared by every list handler."""

import base64
import binascii
import json
import re

from .errors import ApiError

DEFAULT_LIMIT = 20
MAX_LIMIT = 100
_CURSOR_ALPHABET = re.compile(r"^[A-Za-z0-9_-]+$")


def parse_limit(query):
    """Return the page size from query["limit"]: default 20, at most 100, digits only and at least 1."""
    if "limit" not in query:
        return DEFAULT_LIMIT
    text = str(query["limit"])
    if not re.fullmatch(r"[0-9]+", text) or int(text) < 1:
        raise ApiError(400, "invalid_limit", "limit must be a positive integer")
    return min(int(text), MAX_LIMIT)


def parse_order(query):
    """Return "asc" or "desc" from query["order"] (default "asc"), or raise ApiError(400, "invalid_order")."""
    if "order" not in query:
        return "asc"
    if query["order"] not in ("asc", "desc"):
        raise ApiError(400, "invalid_order", "order must be asc or desc")
    return query["order"]


def sort_key(item):
    """Return the ordering key of a record: created_at, then id."""
    return (item["created_at"], item["id"])


def encode_cursor(resource, filters, order, key):
    """Build the opaque cursor for the record with this sort key."""
    payload = json.dumps({"r": resource, "f": filters, "o": order, "k": list(key)}, sort_keys=True, separators=(",", ":"))
    return base64.urlsafe_b64encode(payload.encode("utf-8")).decode("ascii").rstrip("=")


def decode_cursor(text, resource, filters, order):
    """Return the sort key a cursor points after, or raise ApiError(400, "invalid_cursor")."""
    bad = ApiError(400, "invalid_cursor", "cursor is not valid for this request")
    if not _CURSOR_ALPHABET.fullmatch(str(text)):
        raise bad
    try:
        padded = text + "=" * (-len(text) % 4)
        payload = json.loads(base64.urlsafe_b64decode(padded.encode("ascii")).decode("utf-8"))
        if payload["r"] != resource or payload["f"] != filters or payload["o"] != order:
            raise bad
        created_at, record_id = payload["k"]
    except (binascii.Error, ValueError, KeyError, TypeError):
        raise bad
    if not isinstance(created_at, str) or isinstance(record_id, bool) or not isinstance(record_id, (int, str)):
        raise bad
    return (created_at, record_id)


def paginate(resource, items, query, filters):
    """Return (200, {"items", "next_cursor"}) for one page of items.

    resource names the list, filters is the dict of filter values that were applied, and the
    cursor is only valid for the same resource, filters and order. Checks run in the order
    limit, order, cursor.
    """
    limit = parse_limit(query)
    order = parse_order(query)
    after = decode_cursor(query["cursor"], resource, filters, order) if "cursor" in query else None
    ordered = sorted(items, key=sort_key, reverse=(order == "desc"))
    if after is not None:
        ordered = [item for item in ordered if _after(sort_key(item), after, order)]
    page = ordered[:limit]
    next_cursor = encode_cursor(resource, filters, order, sort_key(page[-1])) if len(ordered) > limit else None
    return 200, {"items": page, "next_cursor": next_cursor}


def _after(key, after, order):
    """Return True when key sorts after the cursor position (keys of different id types never match)."""
    if type(key[1]) is not type(after[1]):
        return False
    return key < after if order == "desc" else key > after


def collect_all(handler, query):
    """Call a list handler with query, follow next_cursor to the end and return every item."""
    items = []
    page_query = dict(query)
    while True:
        status, body = handler(page_query)
        items.extend(body["items"])
        if body["next_cursor"] is None:
            return items
        page_query = dict(query, cursor=body["next_cursor"])
