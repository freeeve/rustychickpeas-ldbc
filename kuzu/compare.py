#!/usr/bin/env python3
"""Cross-check the rust engine's query results against Kùzu's.

Compares `<dir>/qN.rust.json` against `<dir>/qN.kuzu.json` (emitted by
`ldbc-bench` with LDBC_EMIT_JSON=<dir> and `run.py ... --emit-json <dir>`).
Rows are sorted canonically first, so result ordering never matters — only the
set of rows and their values. Exits non-zero on any mismatch.

Usage: compare.py <dir> [q1 q2 ...]
"""
import json
import sys


def load(path):
    with open(path) as f:
        return json.load(f)


def main():
    d = sys.argv[1]
    queries = sys.argv[2:] or ["q1", "q2"]
    ok = True
    for q in queries:
        try:
            a = sorted(load(f"{d}/{q}.rust.json"))
            b = sorted(load(f"{d}/{q}.kuzu.json"))
        except FileNotFoundError as e:
            print(f"  {q}: SKIP ({e})")
            continue
        if a == b:
            print(f"  {q}: PASS ({len(a)} rows identical)")
            continue
        ok = False
        print(f"  {q}: FAIL (rust={len(a)} rows, kuzu={len(b)} rows)")
        only_rust = [x for x in a if x not in b][:5]
        only_kuzu = [x for x in b if x not in a][:5]
        if only_rust:
            print(f"      only in rust: {only_rust}")
        if only_kuzu:
            print(f"      only in kuzu: {only_kuzu}")
    print("ALL PASS" if ok else "MISMATCH")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
