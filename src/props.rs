//! Date arithmetic and typed property/graph accessors shared by all query
//! families.

use rustychickpeas_core::{GraphSnapshot, ValueId};

/// Sort a `node -> count` histogram (e.g. from `GraphSnapshot::neighbor_counts`) by
/// count descending, node id ascending on ties, and keep the top `limit` — the
/// selector behind the group-by-count queries. Takes any `(node, count)` iterable
/// so it accepts either hashbrown's or std's `HashMap`.
pub fn top_k_by_count(counts: impl IntoIterator<Item = (u32, usize)>, limit: usize) -> Vec<(u32, usize)> {
    let mut rows: Vec<(u32, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(limit);
    rows
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// algorithm). Used so date-range filters and N-day window arithmetic are plain
/// integer comparisons.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an LDBC creationDate ("2010-02-24T08:06:02.996+00:00") into
/// (year, days-since-epoch).
pub fn parse_date(s: &str) -> Option<(i64, i64)> {
    if s.len() < 10 {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..10].parse().ok()?;
    Some((y, days_from_civil(y, m, d)))
}

/// Parse an LDBC creationDate ("2010-02-24T08:06:02.996+00:00") into epoch
/// milliseconds (for Q17's sub-day timing comparison).
pub fn parse_ms(s: &str) -> i64 {
    let Some((_, day)) = parse_date(s) else {
        return 0;
    };
    let h: i64 = s.get(11..13).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mi: i64 = s.get(14..16).and_then(|x| x.parse().ok()).unwrap_or(0);
    let se: i64 = s.get(17..19).and_then(|x| x.parse().ok()).unwrap_or(0);
    let ms: i64 = if s.len() >= 23 && s.as_bytes()[19] == b'.' {
        s[20..23].parse().unwrap_or(0)
    } else {
        0
    };
    day * 86_400_000 + h * 3_600_000 + mi * 60_000 + se * 1_000 + ms
}

pub fn pi64(g: &GraphSnapshot, n: u32, k: &str) -> i64 {
    match g.prop(n, k) {
        Some(ValueId::I64(v)) => v,
        _ => 0,
    }
}

pub fn pbool(g: &GraphSnapshot, n: u32, k: &str) -> bool {
    matches!(g.prop(n, k), Some(ValueId::Bool(true)))
}

pub fn pstr<'a>(g: &'a GraphSnapshot, n: u32, k: &str) -> Option<&'a str> {
    match g.prop(n, k) {
        Some(ValueId::Str(s)) => g.resolve_string(s),
        _ => None,
    }
}

/// Minimal JSON string escaper (enough for LDBC tag/place names).
/// Find a Tag node by its name property.
pub fn tag_by_name(g: &GraphSnapshot, name: &str) -> Option<u32> {
    g.nodes_with_label("Tag")
        .and_then(|tags| tags.iter().find(|&t| pstr(g, t, "name") == Some(name)))
}
