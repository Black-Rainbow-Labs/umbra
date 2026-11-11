"""Serializes the sample object of every entity and checks the Customer model, printing JSON.

Usage: python dump_serializers.py <package root folder>
"""
import importlib
import json
import pkgutil
import sys

sys.path.insert(0, sys.argv[1])

import shopkit.repositories as repositories  # noqa: E402

output = {"samples": {}, "customer": {}}
for module_info in pkgutil.iter_modules(repositories.__path__):
    entity = module_info.name
    sample = getattr(importlib.import_module(f"shopkit.repositories.{entity}"), f"sample_{entity}")()
    serialize = getattr(importlib.import_module(f"shopkit.serializers.{entity}"), f"serialize_{entity}")
    output["samples"][entity] = serialize(sample)

from shopkit.models.customer import Customer  # noqa: E402
from shopkit.serializers.customer import serialize_customer  # noqa: E402

default_customer = Customer(id=7, name="Grace", email="g@example.com", created_year=2020)
gold_customer = Customer(id=8, name="Alan", email="a@example.com", created_year=2021, loyalty_tier="gold")
output["customer"]["default_tier"] = getattr(default_customer, "loyalty_tier", None)
output["customer"]["default_serialized"] = serialize_customer(default_customer)
output["customer"]["gold_serialized"] = serialize_customer(gold_customer)
print(json.dumps(output, sort_keys=True))
