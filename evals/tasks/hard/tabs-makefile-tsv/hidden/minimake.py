"""A small stand in for make, used by the verifier because make may not be installed.

Usage: python minimake.py -C DIR [--dump] [NAME=value ...] [target ...]

Supported: NAME = value (also := and ?=), $(NAME), $@ $< $^, rules with prerequisites on the
target line, .PHONY, comment lines, backslash continuation, recipe lines that start with a tab
(an optional @ or - prefix is honored). Like make, a line that starts with spaces inside a rule is
a "missing separator" error. Recipes run with sh. A target is rebuilt when it is phony, missing,
or older than a prerequisite.
"""

import json
import os
import re
import shutil
import subprocess
import sys


def find_shell():
    """Return the path of an sh executable (Git for Windows ships one)."""
    found = shutil.which("sh")
    if found:
        return found
    for candidate in [r"C:\Program Files\Git\usr\bin\sh.exe", r"C:\Program Files\Git\bin\sh.exe"]:
        if os.path.exists(candidate):
            return candidate
    sys.exit("minimake: no sh found")


def join_continuations(text):
    """Join lines that end in a backslash with the next line."""
    lines = text.split("\n")
    joined = []
    index = 0
    while index < len(lines):
        line = lines[index]
        while line.endswith("\\") and index + 1 < len(lines):
            index += 1
            line = line[:-1] + " " + lines[index].lstrip()
        joined.append(line)
        index += 1
    return joined


def parse_makefile(text):
    """Parse the Makefile text into variables, phony targets and rules (in file order)."""
    variables = {}
    phony = set()
    rules = []
    current = None
    for number, line in enumerate(join_continuations(text), 1):
        if line.startswith("\t"):
            if current is None:
                sys.exit("Makefile:%d: *** recipe commences before first target.  Stop." % number)
            current["recipe"].append(line[1:])
            continue
        if line.strip() == "" or line.lstrip().startswith("#"):
            continue
        current = None
        assignment = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*(:=|\?=|=)\s*(.*)$", line)
        if assignment:
            name, operator, value = assignment.groups()
            if operator != "?=" or name not in variables:
                variables[name] = value.strip()
            continue
        if ":" in line and not line.startswith(" "):
            targets, prereqs = line.split(":", 1)
            if targets.strip() == ".PHONY":
                phony.update(prereqs.split())
                continue
            current = {"targets": targets.split(), "prereqs": prereqs.split(), "recipe": []}
            rules.append(current)
            continue
        sys.exit("Makefile:%d: *** missing separator.  Stop." % number)
    return variables, phony, rules


def expand(text, variables, depth=0):
    """Expand $(NAME) and ${NAME} references."""
    if depth > 20:
        sys.exit("minimake: variable expansion too deep")

    def replace(match):
        name = match.group(1) or match.group(2)
        return expand(variables.get(name, ""), variables, depth + 1)

    return re.sub(r"\$\(([A-Za-z_][A-Za-z0-9_]*)\)|\$\{([A-Za-z_][A-Za-z0-9_]*)\}", replace, text)


class Runner:
    """Builds targets the way make does, with a much smaller feature set."""

    def __init__(self, directory, variables, phony, rules, shell):
        """Store the parsed Makefile and expand names once."""
        self.directory = directory
        self.variables = variables
        self.shell = shell
        self.phony = set(expand(name, variables) for name in phony)
        self.rules = {}
        for rule in rules:
            prereqs = expand(" ".join(rule["prereqs"]), variables).split()
            for target in expand(" ".join(rule["targets"]), variables).split():
                self.rules[target] = {"prereqs": prereqs, "recipe": rule["recipe"]}
        self.done = set()

    def path(self, name):
        """Return the absolute path of a target name."""
        return os.path.join(self.directory, name)

    def needs_build(self, target, rule):
        """Return True when the target must be rebuilt."""
        if target in self.phony or not os.path.exists(self.path(target)):
            return True
        mtime = os.path.getmtime(self.path(target))
        return any(os.path.exists(self.path(item)) and os.path.getmtime(self.path(item)) > mtime for item in rule["prereqs"])

    def build(self, target):
        """Build a target and its prerequisites; returns False and prints an error on failure."""
        if target in self.done:
            return True
        rule = self.rules.get(target)
        if rule is None:
            if os.path.exists(self.path(target)):
                return True
            print("minimake: *** No rule to make target '%s'.  Stop." % target)
            return False
        for item in rule["prereqs"]:
            if not self.build(item):
                return False
        self.done.add(target)
        if not self.needs_build(target, rule):
            return True
        return self.run_recipe(target, rule)

    def run_recipe(self, target, rule):
        """Run the recipe lines of a rule with sh and stop at the first failing line."""
        extra = {"@": target, "<": rule["prereqs"][0] if rule["prereqs"] else "", "^": " ".join(rule["prereqs"])}
        for raw in rule["recipe"]:
            line = raw
            silent = ignore = False
            while line[:1] in ("@", "-", " "):
                silent = silent or line[0] == "@"
                ignore = ignore or line[0] == "-"
                line = line[1:]
            line = re.sub(r"\$([@<^])", lambda match: extra[match.group(1)], expand(line, self.variables))
            if not silent:
                print(line)
            sys.stdout.flush()
            code = subprocess.run([self.shell, "-c", line], cwd=self.directory).returncode
            if code != 0 and not ignore:
                print("minimake: *** [%s] Error %d" % (target, code))
                return False
        return True


def main():
    """Parse the command line, load the Makefile and build the requested targets."""
    args = sys.argv[1:]
    directory = "."
    dump = False
    overrides = {}
    targets = []
    index = 0
    while index < len(args):
        arg = args[index]
        if arg == "-C":
            directory = args[index + 1]
            index += 1
        elif arg == "--dump":
            dump = True
        elif re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", arg):
            name, value = arg.split("=", 1)
            overrides[name] = value
        else:
            targets.append(arg)
        index += 1
    with open(os.path.join(directory, "Makefile"), "rb") as handle:
        text = handle.read().decode("utf-8")
    variables, phony, rules = parse_makefile(text)
    variables.update(overrides)
    runner = Runner(directory, variables, phony, rules, find_shell())
    if dump:
        print(json.dumps({"phony": sorted(runner.phony), "rules": runner.rules, "variables": variables}))
        return 0
    if not targets:
        targets = [expand(rules[0]["targets"][0], variables)] if rules else []
    for target in targets:
        if not runner.build(target):
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
