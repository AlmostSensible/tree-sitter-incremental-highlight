#!/usr/bin/env python3
"""Repeat the comparison and gate fixture/locality regressions everywhere.
Timing gates require an explicit baseline from the same machine. Hosted CI is noisy.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", nargs="?", default="results/benchmark.json")
    parser.add_argument("--baseline", type=Path,
                        default=ROOT / "benchmarks/baselines/macos-arm64-2026-10-12.json")
    parser.add_argument("--timing", action="store_true", help="enforce 50%% median limit on matching hardware")
    args = parser.parse_args()
    path = Path(args.report).resolve()
    subprocess.run(["cargo", "run", "--locked", "--release", "-p", "highlight-benchmark", "--", str(path)], cwd=ROOT, check=True)
    if os.environ.get("HIGHLIGHT_AUDIT_ONLY"):
        return
    report = json.loads(path.read_text())
    model = subprocess.check_output(["sysctl", "-n", "hw.model"], text=True).strip() if platform.system() == "Darwin" else platform.node()
    architecture = {"aarch64": "arm64", "AMD64": "x86_64"}.get(platform.machine(), platform.machine())
    report.update({
        "machine": {"model": model, "architecture": architecture},
        "os": platform.platform(),
        "measured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)),
    })
    baseline = json.loads(args.baseline.read_text())
    rows = {(r["language"], r["entries"], r["scenario"]): r for r in baseline["workloads"]}
    same_machine = report["machine"] == baseline.get("machine")
    failures = []
    if args.timing and not same_machine:
        failures.append("Timing requested but hardware does not match baseline; supply --baseline from this machine")
    for row in report["workloads"]:
        name = f'{row["language"]}/{row["entries"]}/{row["scenario"]}'
        old = rows.get((row["language"], row["entries"], row["scenario"]))
        if old:
            for key in ("sha256", "edited_sha256"):
                if row[key] != old[key]:
                    failures.append(f"{name}: fixture changed ({key})")
        scenario = row["scenario"]
        if scenario in ("local", "function-local") or scenario.startswith("embedded-"):
            if max(row["queried_bytes"]) >= 500 or max(row["patch_utf16"]) >= 500 or max(row["patch_spans"]) >= 50 or any(row["full_patches"]):
                failures.append(f"{name}: local query/patch limits regressed")
        if scenario == "malformed" and not all(row["full_patches"]):
            failures.append(f"{name}: conservative malformed recovery regressed")
        if args.timing and same_machine:
            if not old:
                failures.append(f"{name}: missing timing baseline")
            else:
                for key in ("ours_edit_ms", "ours_load_ms"):
                    if statistics.median(row[key]) > statistics.median(old[key]) * 1.5:
                        failures.append(f"{name}: {key} regressed more than 50%")
    report["checks"] = {"timing_requested": args.timing, "same_machine": same_machine, "failures": failures}
    path.write_text(json.dumps(report, indent=2) + "\n")
    print("language / entries / scenario | load ms | edit ms | upstream UTF16 edit / ours")
    for row in report["workloads"]:
        load = statistics.median(row["ours_load_ms"])
        edit = statistics.median(row["ours_edit_ms"])
        ratio = statistics.median(row["upstream_utf16_edit_ms"]) / edit
        print(f'{row["language"]}/{row["entries"]}/{row["scenario"]} | {load:.3f} | {edit:.3f} | {ratio:.2f}x')
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Passed {len(report['workloads'])} workloads. Report: {path}")


if __name__ == "__main__":
    main()
