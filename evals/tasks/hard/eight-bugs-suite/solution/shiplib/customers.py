"""Customer email helpers."""


def normalize_email(text):
    """Return a canonical form of an email address, used to match customers.

    The address is trimmed and lowercased. For gmail.com addresses only, dots are removed from the part
    before the @ and everything from the first + up to the @ is removed. Addresses at any other domain
    keep their dots and their + tags.
    """
    local, _, domain = text.strip().lower().partition("@")
    if domain == "gmail.com":
        local = local.split("+")[0].replace(".", "")
    return f"{local}@{domain}"


def dedupe_emails(addresses):
    """Return the addresses with repeats removed, keeping the first one written for each customer.

    Two addresses are repeats when normalize_email gives the same result. The order of the first
    appearances is kept and each address is returned as it was written.
    """
    seen = set()
    unique = []
    for address in addresses:
        key = normalize_email(address)
        if key not in seen:
            seen.add(key)
            unique.append(address)
    return unique
