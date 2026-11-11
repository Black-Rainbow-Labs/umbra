"""Errors that handlers raise and the router turns into HTTP style responses."""


class ApiError(Exception):
    """An error with an HTTP status, a short machine readable code and a message."""

    def __init__(self, status, code, message):
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message
