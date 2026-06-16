//! LDBC SNB Interactive (IC) workload over real SF data. Thin entry point; the
//! seeds, queries, and runner live in `rustychickpeas_ldbc::interactive`.

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::interactive::run()
}
