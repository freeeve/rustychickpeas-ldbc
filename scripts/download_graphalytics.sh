#!/bin/bash
# Download an LDBC Graphalytics dataset (vertices + edges + reference outputs).
#
# Each dataset is a directory of: <name>.v (vertex ids), <name>.e (edges as
# "src dst [weight]"), <name>.properties (per-algorithm run parameters), and one
# reference-output file per algorithm (<name>-BFS, -PR, -WCC, -CDLP, -LCC, -SSSP)
# used to validate, not just time.
#
# VERIFY THE MIRROR: the Graphalytics data hosting has moved over time (SURFsara
# -> Zenodo -> LDBC mirrors), so the base URL is left configurable rather than
# hard-coded blindly. Set GRAPHALYTICS_BASE_URL to the confirmed current mirror
# before running, e.g.:
#   GRAPHALYTICS_BASE_URL=https://<confirmed-mirror>/graphalytics \
#     ./scripts/download_graphalytics.sh example-directed
#
# Start with the tiny "example-directed" / "example-undirected" sets to wire the
# validator, then a small real one (wiki-Talk, kgs). For a zero-download laptop
# run on real SNB topology, see tasks/006 (export the knows subgraph to .v/.e —
# timing only, no reference output).
set -euo pipefail

DATASET="${1:-example-directed}"
DATA_DIR="${2:-data/graphalytics}"
BASE_URL="${GRAPHALYTICS_BASE_URL:-}"

if [[ -z "${BASE_URL}" ]]; then
    echo "ERROR: set GRAPHALYTICS_BASE_URL to the confirmed Graphalytics mirror." >&2
    echo "  See the LDBC Graphalytics docs for the current dataset location." >&2
    exit 1
fi

ARCHIVE="${DATA_DIR}/${DATASET}.tar.zst"
URL="${BASE_URL%/}/${DATASET}.tar.zst"

mkdir -p "${DATA_DIR}"

if [[ ! -f "${ARCHIVE}" ]]; then
    echo "Downloading Graphalytics dataset '${DATASET}' from ${URL}" >&2
    curl -L --retry 5 --retry-connrefused -C - -o "${ARCHIVE}" "${URL}"
else
    echo "Archive already present: ${ARCHIVE}" >&2
fi

echo "Extracting to ${DATA_DIR}/ ..." >&2
tar -x --use-compress-program=unzstd -f "${ARCHIVE}" -C "${DATA_DIR}"

VFILE="${DATA_DIR}/${DATASET}/${DATASET}.v"
if [[ -f "${VFILE}" ]]; then
    echo "Done. Dataset ready at ${DATA_DIR}/${DATASET}/" >&2
    echo "Run: cargo run --release --bin graphalytics -- ${DATA_DIR}/${DATASET}" >&2
else
    echo "ERROR: expected ${VFILE} not found after extract; check the archive layout." >&2
    exit 1
fi
