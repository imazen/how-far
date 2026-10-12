#!/usr/bin/env python3
"""Guard the build cost of how-far and how-far-along.

Fails the run if how-far gains any features, a build script or a dependency other
than `enough`; if a closure, reserved stage or fork branch adds more than --ir-budget
lines of how-far's unoptimized LLVM IR to the caller's crate; or if
how-far-along and how-far-really's own unoptimized IR, grows past its budget.

With `perf`, also reports rustc instructions (as rustc-perf does; they do not
depend on machine load): how-far and how-far-along against an empty no_std
crate, and closure, reserved-stage and fork handoffs against plain function
calls, for check, debug and release builds.
"""
import argparse
import json
import os
import re
import shlex
import shutil
import statistics
import subprocess
import tempfile
import time
from functools import cache
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--runs", type=int, default=3, help="perf samples per measurement")
parser.add_argument("--feature-audit", action="store_true",
                    help="also measure optional APIs separately; default budgets still apply")
parser.add_argument("--wall-time", action="store_true",
                    help="measure rustc elapsed seconds instead of perf instructions")
parser.add_argument("--ir-budget", type=int, default=120, help="IR lines per call site")
parser.add_argument("--along-budget", type=int, nargs=2, default=[25_000, 35_000],
                    metavar=("TRACKER", "DIAGNOSTICS"), help="tracker and diagnostics IR lines")
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent

meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version=1", "--no-deps"], cwd=root))
how_far = next(p for p in meta["packages"] if p["name"] == "how-far")
assert how_far["features"] == {}, "core exposes its full API without Cargo features"
assert [(d["name"], d["kind"]) for d in how_far["dependencies"] if d["kind"] != "dev"] \
    == [("enough", None)], "how-far may depend only on the lightweight enough interface"
assert all("custom-build" not in t["kind"] for t in how_far["targets"]), \
    "how-far must not have a build script"
print(subprocess.check_output(["rustc", "--version"], text=True).strip())

WORK = """
#[inline(never)]
fn work{i}(data: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {{
    let pulse = pulse.live();
    let mut sum = 0_u64;
    for chunk in data.chunks_mut(64) {{
        for byte in chunk.iter_mut() {{
            *byte = byte.wrapping_mul({m}).rotate_left(3);
            sum = sum.wrapping_add(u64::from(*byte));
        }}
        pulse.step(1)?;
    }}
    Ok(sum)
}}
"""


def probe(n, stages, direct=False):
    body = "use how_far::prelude::*;\nuse how_far::StopReason;\n"
    body += "".join(WORK.format(i=i, m=2 * i + 3) for i in range(n))
    if stages:
        body += "use how_far::{PhaseSpec, Stages, Total};\n"
        plan = ", ".join(f'PhaseSpec::new("s{i}", 1, Total::Unknown)' for i in range(n))
        if direct:
            calls = "".join(f'    let stage = stages.begin(s{i});\n'
                            f"    sum += work{i}(data, &stage).finish_phase(stage)?;\n"
                            for i in range(n))
            return body + ("pub fn run(data: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {\n"
                           f"    let (mut stages, [{', '.join(f's{i}' for i in range(n))}]) = Stages::define(pulse, [{plan}]);\n"
                           f"    let mut sum = 0;\n{calls}"
                           "    stages.complete(Ok(sum))\n}\n")
        calls = "".join(f"    sum += stages.run(|stage| work{i}(data, stage))?;\n"
                        for i in range(n))
        return body + ("pub fn run(data: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {\n"
                       f"    let mut stages = Stages::new(pulse, &[{plan}]);\n"
                       f"    let result = (|| {{ let mut sum = 0;\n{calls}    Ok(sum) }})();\n"
                       "    result.finish_phase(stages)\n}\n")
    calls = "".join(f"    sum += work{i}(data, pulse)?;\n" for i in range(n))
    return body + ("pub fn run(data: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {\n"
                   f"    let mut sum = 0;\n{calls}    Ok(sum)\n}}\n")


def fork_probe(n):
    # Scheduling is deliberately absent: this measures the ownership/handoff
    # interface independently of a threading library's monomorphizations.
    body = "use how_far::{prelude::*, PhaseSpec, Stages, StopReason, Total};\n"
    body += "".join(WORK.format(i=i, m=2 * i + 3) for i in range(n))
    names = ", ".join(f"s{i}" for i in range(n))
    specs = ", ".join(f'PhaseSpec::new("s{i}", 1, Total::Unknown)' for i in range(n))
    calls = "".join(f"    let r{i} = work{i}(data, &s{i}).finish_phase(s{i});\n" for i in range(n))
    results = ", ".join(f"r{i}" for i in range(n))
    return body + (
        "pub fn run(data: &mut [u8], pulse: &dyn Pulse) -> Result<u64, StopReason> {\n"
        '    let (mut stages, [work]) = Stages::define(pulse, [PhaseSpec::new("work", 1, Total::Unknown)]);\n'
        "    let work = stages.begin(work);\n"
        f"    let [{names}] = work.fork([{specs}]);\n"
        f"{calls}    let result: Result<u64, StopReason> = [{results}].into_iter().sum();\n"
        "    let sum = result.finish_phase(work)?;\n"
        "    stages.complete(Ok(sum))\n}\n")


SMALL, LARGE = 8, 32
PROBES = {
    "empty": "#![no_std]\nextern crate alloc;\npub use enough::Stop;\n",
    f"plain{SMALL}": probe(SMALL, False), f"plain{LARGE}": probe(LARGE, False),
    f"stages{SMALL}": probe(SMALL, True), f"stages{LARGE}": probe(LARGE, True),
    f"direct{SMALL}": probe(SMALL, True, True), f"direct{LARGE}": probe(LARGE, True, True),
    f"fork{SMALL}": fork_probe(SMALL), f"fork{LARGE}": fork_probe(LARGE),
}
PROFILES = {"check": ["check"], "debug": ["build"], "release": ["build", "--release"]}
# how-far-along feature sets, each built in its own workspace so cargo does
# not unify their features.
ALONG = {
    "std": ("how-far-along", []),
    "diagnostics": ("how-far-really", []),
}
if args.feature_audit:
    ALONG.update({
        "smooth": ("how-far-along", ["smooth"]),
        "stage-suggestions": ("how-far-really", ["experimental-stage-suggestions"]),
    })


def perf_instructions(argv, cwd):
    out = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "--", *argv],
                         cwd=cwd, capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr[-2000:])
    return int(next(l for l in out.stderr.splitlines() if "instructions" in l).split(",")[0])


def wall_seconds(argv, cwd):
    start = time.perf_counter()
    result = subprocess.run(argv, cwd=cwd, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return time.perf_counter() - start


with tempfile.TemporaryDirectory(prefix="how-far-build-") as tmp:
    tmp = Path(tmp)
    for name, source in PROBES.items():
        (tmp / name / "src").mkdir(parents=True)
        (tmp / name / "src" / "lib.rs").write_text(source)
        (tmp / name / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\n'
            f'enough = {{ version = "0.4.5", default-features = false, features = ["alloc"] }}\n'
            f'how-far = {{ path = "{root}/crates/how-far" }}\n')
    (tmp / "Cargo.toml").write_text(
        "[workspace]\nresolver = \"3\"\nmembers = [" + ", ".join(f'"{n}"' for n in PROBES) + "]\n")
    env = dict(os.environ, CARGO_TARGET_DIR=str(tmp / "target"), CARGO_INCREMENTAL="0")
    for name, (crate, features) in ALONG.items():
        (tmp / "along" / name / "src").mkdir(parents=True)
        (tmp / "along" / name / "src" / "lib.rs").write_text("")
        (tmp / "along" / name / "Cargo.toml").write_text(
            f'[package]\nname = "along-{name}"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\n'
            f'{crate} = {{ path = "{root}/crates/{crate}", features = {json.dumps(features)} }}\n[workspace]\n')
    commands = {}
    for profile, cargo_args in PROFILES.items():
        for cwd, tag in [(tmp, ""), *((tmp / "along" / name, f"/{name}") for name in ALONG)]:
            log = subprocess.run(["cargo", *cargo_args, "--offline", "-v", "--color=never", "--workspace"],
                                 cwd=cwd, env=env, capture_output=True, text=True, check=True).stderr
            for line in log.splitlines():
                if "Running `" in line and "--crate-name" in line:
                    argv = shlex.split(line.split("Running `", 1)[1].rstrip("`"))
                    argv = [a for a in argv if not a.startswith("--json") and a != "--error-format=json"]
                    crate = argv[argv.index("--crate-name") + 1]
                    if tag and crate == ALONG[tag[1:]][0].replace("-", "_"):
                        commands[profile, "observation" + tag] = argv
                    elif not tag:  # how-far as a library sees it: enough without std.
                        commands[profile, crate] = argv

    @cache
    def ir_functions(name, profile="debug"):
        argv = [a for a in commands[profile, name] if not a.startswith("--emit")]
        out = tmp / "ir" / profile / name.replace("/", "-")
        out.mkdir(parents=True)
        argv[argv.index("--out-dir") + 1] = str(out)
        subprocess.run([*argv, "--emit=llvm-ir"], cwd=tmp, env=env, check=True, capture_output=True)
        ir = next(out.glob("*.ll")).read_text()
        return [(m.group(1), m.group(0)) for m in
                re.finditer(r"^define [^\n]*?@(\S+?)\(.*?\n}\n", ir, re.S | re.M)]

    def ir_lines(name, only_how_far=True):
        """Unoptimized IR lines, without debug-info records, of a debug build."""
        return sum(
            sum(1 for line in body.splitlines()[1:] if line.strip() and not line.lstrip().startswith("#dbg"))
            for symbol, body in ir_functions(name)
            if not only_how_far or "7how_far" in symbol)

    def checkpoint_ir(name):
        found = {}
        for symbol, body in ir_functions(name, "release"):
            if "PulseTree" not in symbol:
                continue
            for method in ["check", "may_stop"]:
                # Rust 1.88 uses legacy mangling; newer compilers use v0.
                if not re.search(rf"Stop.*{len(method)}{method}", symbol):
                    continue
                # Inspect instructions without comments or debug records.
                lines = [line.strip() for line in body.splitlines()[1:]
                         if line.strip() and not line.lstrip().startswith((";", "#dbg"))]
                instructions = "\n".join(lines)
                assert "br i1" not in instructions, f"ordinary {method} gained a conditional branch"
                assert method not in found, f"ambiguous PulseTree::{method} symbol"
                found[method] = instructions
        assert set(found) == {"check", "may_stop"}, "missing ordinary checkpoint symbols"
        return found

    checkpoint_ir("observation/std")
    print("PulseTree check/may_stop: branch-free release IR with unconditional callbacks")

    per_site = (ir_lines(f"stages{LARGE}") - ir_lines(f"stages{SMALL}")) / (LARGE - SMALL)
    print(f"how-far IR per Stages::run call site: {per_site:.0f} lines "
          f"(budget {args.ir_budget})")
    assert per_site <= args.ir_budget, "Stages::run instantiates too much code per call site"
    direct_site = (ir_lines(f"direct{LARGE}") - ir_lines(f"direct{SMALL}")) / (LARGE - SMALL)
    print(f"how-far IR per reserved stage call site: {direct_site:.0f} lines "
          f"(budget {args.ir_budget})")
    assert direct_site <= args.ir_budget, "reserved stage handoff instantiates too much code per call site"
    fork_site = (ir_lines(f"fork{LARGE}") - ir_lines(f"fork{SMALL}")) / (LARGE - SMALL)
    print(f"how-far IR per fork branch call site: {fork_site:.0f} lines "
          f"(budget {args.ir_budget})")
    assert fork_site <= args.ir_budget, "fork handoff instantiates too much code per call site"
    for name, budget in zip(ALONG, args.along_budget):
        lines = ir_lines(f"observation/{name}", only_how_far=False)
        print(f"{ALONG[name][0]} IR, {name}: {lines} lines (budget {budget})")
        assert lines <= budget, f"how-far-along with {name} compiles to more code than its budget"

    if args.feature_audit:
        print(f"how-far own IR, default: {ir_lines('how_far', only_how_far=False)} lines")
        for name in list(ALONG)[2:]:
            lines = ir_lines(f"observation/{name}", only_how_far=False)
            print(f"{ALONG[name][0]} own IR, {name}: {lines} lines (audit, not a default-budget gate)")

    if not args.wall_time and shutil.which("perf") is None:
        print("perf not found; skipping instruction counts")
        raise SystemExit(0)
    try:
        crates = ["how_far", *PROBES, *(f"observation/{name}" for name in ALONG)]
        samples = {(p, c): [] for p in PROFILES for c in crates}
        # Replays must not overwrite the dependency metadata used by another
        # profile or feature set. Keep Cargo's artifacts as immutable inputs.
        for profile, crate in samples:
            out = tmp / "replay" / profile / crate.replace("/", "-")
            out.mkdir(parents=True)
            argv = commands[profile, crate]
            argv[argv.index("--out-dir") + 1] = str(out)
        for _ in range(args.runs):
            for key in samples:
                measure = wall_seconds if args.wall_time else perf_instructions
                samples[key].append(measure(commands[key], tmp))
    except RuntimeError:
        if args.wall_time:
            raise
        print("perf cannot count instructions here (see kernel.perf_event_paranoid); skipping them")
        raise SystemExit(0)
    scale = 1 if args.wall_time else 1e6
    precision = 3 if args.wall_time else 1
    m = {key: statistics.median(values) / scale for key, values in samples.items()}
    units = "rustc elapsed seconds (warm files)" if args.wall_time else "rustc instructions (millions)"
    print(f"{units:44} {'check':>7} {'debug':>7} {'release':>7}")
    rows = {
        "empty no_std crate": lambda p: m[p, "empty"],
        "how-far": lambda p: m[p, "how_far"],
        "how-far-along (std)": lambda p: m[p, "observation/std"],
        "how-far-really": lambda p: m[p, "observation/diagnostics"],
        "plain function call, per call site": lambda p:
            (m[p, f"plain{LARGE}"] - m[p, f"plain{SMALL}"]) / (LARGE - SMALL),
        "reserved stage, per call site": lambda p:
            (m[p, f"direct{LARGE}"] - m[p, f"direct{SMALL}"]) / (LARGE - SMALL),
        "fork branch, per call site": lambda p:
            (m[p, f"fork{LARGE}"] - m[p, f"fork{SMALL}"]) / (LARGE - SMALL),
        "Stages::run, per call site": lambda p:
            (m[p, f"stages{LARGE}"] - m[p, f"stages{SMALL}"]) / (LARGE - SMALL),
    }
    for label, value in rows.items():
        print(f"{label:44} " + " ".join(f"{value(p):7.{precision}f}" for p in PROFILES))
    if args.feature_audit:
        for name in list(ALONG)[2:]:
            print(f"{name:44} " + " ".join(
                f"{m[p, 'observation/' + name]:7.{precision}f}" for p in PROFILES))
