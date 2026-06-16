//! LDBC SPB benchmark: load RDF (N-Triples) into rustychickpeas as a property
//! graph and run hand-coded SPB-style queries (no SPARQL engine). Thin entry
//! point; the work lives in `rustychickpeas_ldbc::spb`.

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::spb::run()
}
