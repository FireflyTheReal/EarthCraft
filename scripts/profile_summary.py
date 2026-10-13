"""Summarize opt-in timings, retaining raster dimensions and phase boundaries."""
import argparse
import csv
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path


def summarize(rows):
    groups = defaultdict(list)
    for row in rows:
        key = (row["operation"], int(row["width"]), int(row["height"]))
        groups[key].append(int(row["elapsed_us"]) / 1000)
    timings = []
    for (operation, width, height), values in sorted(groups.items()):
        values.sort()
        timings.append({
            "operation": operation,
            "width": width,
            "height": height,
            "samples": len(values),
            "median_ms": round(statistics.median(values), 3),
            "p95_ms": round(values[max(0, math.ceil(len(values) * 0.95) - 1)], 3),
            "max_ms": round(values[-1], 3),
        })
    return {
        "timings": timings,
        "max_inflight": max((int(row["inflight"]) for row in rows), default=0),
        "max_decoded_estimate_mib": round(max((int(row["resident_bytes"]) for row in rows), default=0) / 1048576, 2),
        "dropped_events_counter": max((int(row["dropped_events"]) for row in rows), default=0),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--phases", type=Path, help="JSON array: phase, start_us, end_us")
    args = parser.parse_args()
    with args.capture.open(newline="") as source:
        rows = [row for row in csv.DictReader(source) if all(row.get(key) for key in ("unix_us", "operation", "elapsed_us", "width", "height", "inflight", "resident_bytes", "dropped_events"))]
    phases = json.loads(args.phases.read_text()) if args.phases else [{"phase": "all", "start_us": 0, "end_us": 2**64}]
    report = {}
    for phase in phases:
        selected = [row for row in rows if phase["start_us"] <= int(row["unix_us"]) < phase.get("end_us", 2**64)]
        report[phase["phase"]] = summarize(selected)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
