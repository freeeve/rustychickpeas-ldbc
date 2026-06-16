#!/bin/bash
# Build a self-contained SPB N-Triples extract = generated creative works + the
# geonames places they reference (coords cast to xsd:double), so rustychickpeas /
# Kùzu can load it without the full 1.5 GB reference data.
#
# Prereqs (see the spb-real-data-pipeline memory): SPB-10 generated under
# data/spb/.../dist/generated, and the Oxigraph server running on :7878 with the
# reference data loaded.
#
#   scripts/spb_extract.sh [N|all] [out.nt]      # N = number of work files
set -euo pipefail

N="${1:-20}"
OUT="${2:-data/spb/extract/spb-${N}.nt}"
GEN=data/spb/ldbc_spb_bm_2.0/dist/generated
EXTRACT=data/spb/extract
PLACES="$EXTRACT/places.nt"
mkdir -p "$EXTRACT"

# 1) Places (once): geonames Features with wgs84 lat/long cast to xsd:double
#    (they are xsd:string in source, which our f64-only geo index would ignore).
if [[ ! -s "$PLACES" ]]; then
    echo "Extracting geonames places from Oxigraph ..." >&2
    curl -s -X POST http://localhost:7878/query --data-urlencode 'query=
PREFIX wgs:<http://www.w3.org/2003/01/geo/wgs84_pos#>
PREFIX go:<http://www.geonames.org/ontology#>
PREFIX xsd:<http://www.w3.org/2001/XMLSchema#>
CONSTRUCT { ?f a go:Feature . ?f wgs:lat ?la . ?f wgs:long ?lo . ?f go:name ?n }
WHERE { ?f a go:Feature ; wgs:lat ?lat ; wgs:long ?lon .
        BIND(xsd:double(?lat) AS ?la) BIND(xsd:double(?lon) AS ?lo)
        OPTIONAL { ?f go:name ?n } }' \
        -H 'Accept: application/n-triples' -o "$PLACES"
fi

# 2) Works: strip the n-quads context (chomp first — a trailing \s*$ eats the
#    newline) and add a cwork:CreativeWork supertype per work (the data types
#    them only as BlogPost/NewsItem/... subclasses).
if [[ "$N" == "all" ]]; then
    FILES=$(ls "$GEN"/generatedCreativeWorks-*.nq)
else
    FILES=$(ls "$GEN"/generatedCreativeWorks-*.nq | head -n "$N")
fi
perl -ne '
  chomp; s/\s+<[^>]*>\s+\.$/ ./; print "$_\n";
  if (/^(<http:\/\/www\.bbc\.co\.uk\/things\/\S+>) <http:\/\/www\.w3\.org\/1999\/02\/22-rdf-syntax-ns#type> <http:\/\/www\.bbc\.co\.uk\/ontologies\/creativework\/\w+> \.$/) {
    print "$1 <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .\n";
  }' $FILES > "$EXTRACT/works.nt"

cat "$EXTRACT/works.nt" "$PLACES" > "$OUT"
echo "Wrote $OUT — $(wc -l < "$OUT") triples (works $(wc -l < "$EXTRACT/works.nt") + places $(wc -l < "$PLACES"))" >&2
