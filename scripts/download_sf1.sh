#!/bin/bash
# Download and extract the real LDBC SNB BI SF1 dataset (compressed CSV).
#
# Uses the current official LDBC mirror. The dataset is ~206 MB compressed and
# extracts to ~210 MB of gzipped CSV part files (the CSVs stay gzip-compressed
# on disk; the loader reads them directly).
set -euo pipefail

DATA_DIR="${1:-data}"
URL="https://datasets.ldbcouncil.org/bi-pre-audit/bi-sf1-composite-merged-fk.tar.zst"
ARCHIVE="${DATA_DIR}/bi-sf1-composite-merged-fk.tar.zst"

mkdir -p "${DATA_DIR}"

if [[ ! -f "${ARCHIVE}" ]]; then
    echo "Downloading SF1 dataset (~206 MB) from ${URL}" >&2
    curl -L --retry 5 --retry-connrefused -C - -o "${ARCHIVE}" "${URL}"
else
    echo "Archive already present: ${ARCHIVE}" >&2
fi

echo "Extracting to ${DATA_DIR}/ ..." >&2
tar -x --use-compress-program=unzstd -f "${ARCHIVE}" -C "${DATA_DIR}"

SNAPSHOT="${DATA_DIR}/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot"
if [[ -d "${SNAPSHOT}/dynamic" ]]; then
    echo "Done. initial_snapshot ready at:" >&2
    echo "  ${SNAPSHOT}" >&2
    echo "Run: cargo run --release" >&2
else
    echo "ERROR: expected snapshot dir not found at ${SNAPSHOT}" >&2
    exit 1
fi
