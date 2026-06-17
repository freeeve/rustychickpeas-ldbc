//! Date arithmetic and typed property/graph accessors shared by all query
//! families.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rustychickpeas_core::{GraphSnapshot, ValueId};

/// Sort a `node -> count` histogram (e.g. from `GraphSnapshot::neighbor_counts`) by
/// count descending, node id ascending on ties, and keep the top `limit` — the
/// selector behind the group-by-count queries. Takes any `(node, count)` iterable
/// so it accepts either hashbrown's or std's `HashMap`.
pub fn top_k_by_count(counts: impl IntoIterator<Item = (u32, usize)>, limit: usize) -> Vec<(u32, usize)> {
    top_k_by_key(counts, limit)
}

/// Rank `(id, key)` rows by key descending, id ascending on ties, and keep the top
/// `limit` — the date/score analog of [`top_k_by_count`]. `key` is any `Ord` carried
/// alongside the row id (a `dateModified` string, a score, a distinct-day count, …);
/// the id is itself any `Ord` — a dense node id for the per-node queries, or a
/// resolved uri / type-name `String` for the queries that group by label.
pub fn top_k_by_key<T: Ord, K: Ord>(rows: impl IntoIterator<Item = (T, K)>, limit: usize) -> Vec<(T, K)> {
    let mut rows: Vec<(T, K)> = rows.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(limit);
    rows
}

/// A streaming top-`k` accumulator: [`push`](Self::push) any number of items and
/// keep only the `k` largest by `Ord`, without materialising the rest — the
/// `BinaryHeap<Reverse<…>>` idiom the IC queries repeat (e.g. "top 20 messages by
/// date desc, id asc"). The streaming complement to [`top_k_by_key`], for when the
/// candidates are produced in a scan rather than collected up front. Embed a
/// [`Reverse`] in the key to flip a tie-break field: `(ms, Reverse(id))` keeps the
/// highest `ms`, and the lowest `id` among equal `ms`.
pub struct TopK<T: Ord> {
    k: usize,
    heap: BinaryHeap<Reverse<T>>,
}

impl<T: Ord> TopK<T> {
    /// A top-`k` accumulator (keeps the `k` largest items offered).
    pub fn new(k: usize) -> Self {
        TopK { k, heap: BinaryHeap::with_capacity(k + 1) }
    }

    /// Offer `item`; kept iff it ranks among the `k` largest seen so far (when
    /// full, it displaces the current smallest kept item if larger).
    pub fn push(&mut self, item: T) {
        if self.heap.len() < self.k {
            self.heap.push(Reverse(item));
        } else if let Some(Reverse(smallest)) = self.heap.peek() {
            if item > *smallest {
                self.heap.pop();
                self.heap.push(Reverse(item));
            }
        }
    }

    /// The kept items, largest first.
    pub fn into_sorted_desc(self) -> Vec<T> {
        let mut v: Vec<T> = self.heap.into_iter().map(|Reverse(t)| t).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v
    }
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

/// Parse the leading `YYYY-MM-DD` of an ISO-8601 date into calendar components
/// `(year, month, day)`; `None` if the string is too short or non-numeric. The
/// component analog of [`parse_date`] — use it when the calendar fields are the
/// group-by / render key rather than a sortable day number.
pub fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    if s.len() < 10 {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    Some((year, month, day))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topk_keeps_largest_in_desc_order() {
        let mut top = TopK::new(3);
        for x in [5, 1, 9, 3, 7, 2, 8] {
            top.push(x);
        }
        assert_eq!(top.into_sorted_desc(), vec![9, 8, 7]);
    }

    #[test]
    fn topk_reverse_tiebreak_keeps_highest_then_lowest_id() {
        // (score, Reverse(id)): top by score desc, id asc on ties.
        let mut top = TopK::new(2);
        for (s, id) in [(5, 10u32), (5, 3), (5, 7), (1, 1)] {
            top.push((s, Reverse(id)));
        }
        let got: Vec<(i32, u32)> =
            top.into_sorted_desc().into_iter().map(|(s, Reverse(id))| (s, id)).collect();
        assert_eq!(got, vec![(5, 3), (5, 7)]);
    }

    #[test]
    fn topk_k_zero_and_underfull() {
        let mut z: TopK<i32> = TopK::new(0);
        z.push(1);
        assert!(z.into_sorted_desc().is_empty());
        let mut u = TopK::new(5);
        for x in [3, 1, 2] {
            u.push(x);
        }
        assert_eq!(u.into_sorted_desc(), vec![3, 2, 1]);
    }
}
