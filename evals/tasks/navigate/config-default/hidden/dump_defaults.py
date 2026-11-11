"""Calls every settings reader in the shopkit package and prints the results as JSON.

Usage: python dump_defaults.py <package root folder> [overrides json]
"""
import importlib
import inspect
import json
import pkgutil
import sys

root = sys.argv[1]
sys.path.insert(0, root)

import shopkit  # noqa: E402
from shopkit import config  # noqa: E402

if len(sys.argv) > 2:
    config.SETTINGS.update(json.loads(sys.argv[2]))

results = {}
for module_info in pkgutil.walk_packages(shopkit.__path__, "shopkit."):
    module = importlib.import_module(module_info.name)
    for name, function in inspect.getmembers(module, inspect.isfunction):
        if function.__module__ != module.__name__ or name == "get_setting":
            continue
        results[f"{module.__name__}.{name}"] = function()

print(json.dumps(results, sort_keys=True))
