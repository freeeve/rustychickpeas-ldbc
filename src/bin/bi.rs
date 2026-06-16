//! Faithful LDBC SNB BI benchmark over real SF data. Thin entry point; the
//! loader, helpers, and queries live in `rustychickpeas_ldbc::bi`.

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::bi::run()
}
