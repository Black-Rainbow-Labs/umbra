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


def sort_key(item):
    """Return the ordering key of a record: created_at, then id."""
    return (item["created_at"], item["id"])


def encode_cursor(resource, filters, key):
    """Build the opaque cursor for the record with this sort key."""
    payload = json.dumps({"r": resource, "f": filters, "k": list(key)}, sort_keys=True, separators=(",", ":"))
    return base64.urlsafe_b64encode(payload.encode("utf-8")).decode("ascii").rstrip("=")


def decode_cursor(text, resource, filters):
    """Return the sort key a cursor points after, or raise ApiError(400, "invalid_cursor")."""
    bad = ApiError(400, "invalid_cursor", "cursor is not valid for this request")
    if not _CURSOR_ALPHABET.fullmatch(str(text)):
        raise bad
    try:
        padded = text + "=" * (-len(text) % 4)
        payload = json.loads(base64.urlsafe_b64decode(padded.encode("ascii")).decode("utf-8"))
        if payload["r"] != resource or payload["f"] != filters:
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
    cursor is only valid for the same resource and the same filters. limit is checked first.
    """
    limit = parse_limit(query)
    after = decode_cursor(query["cursor"], resource, filters) if "cursor" in query else None
    ordered = sorted(items, key=sort_key)
    if after is not None:
        ordered = [item for item in ordered if _after(sort_key(item), after)]
    page = ordered[:limit]
    next_cursor = encode_cursor(resource, filters, sort_key(page[-1])) if len(ordered) > limit else None
    return 200, {"items": page, "next_cursor": next_cursor}


def _after(key, after):
    """Return True when key sorts after the cursor position (keys of different id types never match)."""
    if type(key[1]) is not type(after[1]):
        return False
    return key > after
