#!/usr/bin/env python3
"""Check that the baseline comparison still fails a member that regressed.

The verdict is the whole net: it decides what counts as "the framework broke a
produced application". It compares against the run a member was accepted from
rather than against an all-green ideal, which is what lets a member carrying
authored expected-fail cases or standout#467's eight invariant cells be frozen
at all — and is exactly the loosening that could go too far and stop failing.

So: take a real member's baseline, mutate one thing at a time, and check the
verdict answers each mutation the way it claims to.
"""

import copy
import json
import pathlib
import sys

from build_member import Failed, verdict

BASELINE = pathlib.Path(__file__).resolve().parent.parent / "members" / "kubelike" / "baseline-report.json"


def check(name: str, report: dict, baseline: dict, should_fail: bool) -> bool:
    try:
        verdict(report, baseline)
        failed = False
    except Failed as error:
        failed = True
        message = str(error)
    if failed != should_fail:
        verb = "should have failed" if should_fail else "should have passed"
        print(f"  FAIL {name}: {verb}", file=sys.stderr)
        return False
    if failed:
        print(f"  ok   {name} -> red: {message.splitlines()[0]}")
    else:
        print(f"  ok   {name} -> green")
    return True


def main() -> int:
    baseline = json.loads(BASELINE.read_text())
    ok = True

    ok &= check("unchanged", copy.deepcopy(baseline), baseline, should_fail=False)

    # A case that passed when accepted now fails: the regression the net exists for.
    broken = copy.deepcopy(baseline)
    broken["acceptance"]["cases"][0]["outcome"] = "fail"
    ok &= check("a passing case now fails", broken, baseline, should_fail=True)

    # An invariant cell that passed when accepted now fails.
    broken = copy.deepcopy(baseline)
    passing = next(c for c in broken["invariants"] if c["status"] == "pass")
    passing["status"] = "fail"
    ok &= check("a passing invariant cell now fails", broken, baseline, should_fail=True)

    # The eight cells standout#467 explains stay failing: not a regression.
    ok &= check("the accepted invariant failures persist", copy.deepcopy(baseline), baseline, should_fail=False)

    # One of them starts passing: movement worth reporting, not worth failing.
    improved = copy.deepcopy(baseline)
    next(c for c in improved["invariants"] if c["status"] == "fail")["status"] = "pass"
    ok &= check("a known invariant failure now passes", improved, baseline, should_fail=False)

    # A gap tripwire whose premise stopped holding: reported, not red. standout's
    # own gaps.toml ledger test is the alarm for that; this build is the alarm
    # for a produced application that stopped working.
    improved = copy.deepcopy(baseline)
    improved["acceptance"]["cases"][0]["outcome"] = "unexpected-pass"
    baseline_gap = copy.deepcopy(baseline)
    baseline_gap["acceptance"]["cases"][0]["outcome"] = "expected-fail"
    baseline_gap["acceptance"]["cases"][0]["expected"] = "fail"
    ok &= check("an expected-fail became unexpected-pass", improved, baseline_gap, should_fail=False)

    # ...but the same gap case actually failing to build its premise is still red.
    broken = copy.deepcopy(baseline)
    broken["acceptance"]["cases"][1]["outcome"] = "fail"
    ok &= check("a second passing case now fails", broken, baseline, should_fail=True)

    # The suite lost a case the member was accepted on.
    shrunk = copy.deepcopy(baseline)
    shrunk["acceptance"]["cases"].append({"name": "invented-later", "expected": "pass", "outcome": "pass"})
    ok &= check("a case the accepted run never had", shrunk, baseline, should_fail=True)

    # A build failure is a build failure.
    unbuilt = copy.deepcopy(baseline)
    unbuilt["acceptance"]["built"] = False
    ok &= check("the app did not build", unbuilt, baseline, should_fail=True)

    print("[corpus] verdict rules hold" if ok else "[corpus] verdict rules BROKEN", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
