#!/usr/bin/env python3
"""Tables for the stage-suggestions record, from committed traces and logs.

usage: summarize.py traces DIR         per case: wall, unchecked head and tail,
                                       and each crate's checked intervals
       summarize.py compare LOG LOG    suggested stages of two runs, side by side

A location's time is the intervals that end at its calls, so a crate's share
is of intervals ending at that crate's checks, not time spent inside it. Time
before the first timed call is the unchecked head; time after the last is the
unchecked tail.
"""
import json
import pathlib
import re
import sys


def package(file):
    parts = re.split(r"[\\/]", file)
    for i in range(len(parts) - 1, 0, -1):
        if parts[i] in ("src", "tests", "benches", "examples"):
            name = parts[i - 1]
            if len(name) >= 7 and re.fullmatch(r"[0-9a-fA-F]+", name) and i >= 2:
                name = parts[i - 2].rsplit("-", 1)[0]
            return re.sub(r"-\d.*$", "", name)
    return "(unknown)"


def traces(directory):
    print("| Case | Wall ms | Checks | Unchecked head ms | Unchecked tail ms | Checked intervals by crate (% of wall) |")
    print("| --- | ---: | ---: | ---: | ---: | --- |")
    for path in sorted(pathlib.Path(directory).glob("*.json")):
        trace = json.loads(path.read_text())
        span = trace["spans"][0]
        start, end = int(span["start"]), int(span["end"])
        wall = end - start
        calls = [s for s in span["sites"] if s["active"] is not None]
        first = min(int(s["active"][0]) for s in calls)
        last = max(int(s["active"][1]) for s in calls)
        crates = {}
        for site in span["sites"]:
            name = package(site["file"])
            crates[name] = crates.get(name, 0) + int(site["time"])
        shares = ", ".join(
            f"{name} {100 * t / wall:.1f}"
            for name, t in sorted(crates.items(), key=lambda kv: -kv[1])
        )
        case = trace.get("metadata", {}).get("case", path.stem)
        print(
            f"| {case} | {wall / 1e6:.1f} | {span['checks']} | "
            f"{(first - start) / 1e6:.1f} | {(end - last) / 1e6:.1f} | {shares} |"
        )


def stages(log):
    cases, current = {}, None
    for line in pathlib.Path(log).read_text().splitlines():
        m = re.match(r"=== (\S+)", line)
        if m:
            current = m.group(1)
            cases[current] = []
            continue
        m = re.search(r'PhaseSpec::new\("(after )?.*?([^/\\]+:\d+)", (\d+),', line)
        if m and current:
            cases[current].append(f"{m.group(1) or ''}{m.group(2)} {m.group(3)}")
    return cases


def compare(first, second):
    a, b = stages(first), stages(second)
    print("| Case | Run 1 | Run 2 | Same stages |")
    print("| --- | --- | --- | --- |")
    for case in a:
        if case not in b:
            continue
        names = lambda run: [s.rsplit(" ", 1)[0] for s in run]
        same = "yes" if names(a[case]) == names(b[case]) else "no"
        print(f"| {case} | {', '.join(a[case]) or 'none'} | {', '.join(b[case]) or 'none'} | {same} |")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "traces":
        traces(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == "compare":
        compare(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)
