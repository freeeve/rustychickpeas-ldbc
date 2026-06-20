//! SPB basic **q9** — most-recent creative works *related* to a given one, ranked
//! by a weighted count of the tagged entities they share.
//!
//! Hand translation of `basic/aggregation_standard/query9.txt`. The official
//! query unions four co-occurrence counts and weights each by *how* the focal work
//! and the other work each tag the shared entity (`cwork:tag` = `cwork:about` ∪
//! `cwork:mentions`):
//!
//! ```text
//! score = ?cnt_2 * 2 + ?cnt_1_5 * 1.5 + ?cnt_1 * 1 + ?cnt_0_5 * 0.5
//! ```
//!
//! Reading each sub-select's body, the count is over entities `E` shared between
//! the focal work and an other work, keyed by the (focal rel, other rel) pair:
//!   * `cnt_2`   — focal `about` E,    other `about` E      → factor **2**
//!   * `cnt_1_5` — focal `about` E,    other `mentions` E   → factor **1.5**
//!   * `cnt_1`   — focal `mentions` E, other `about` E      → factor **1**
//!   * `cnt_0_5` — focal `mentions` E, other `mentions` E   → factor **0.5**
//!
//! so the focal work's `about` links and the other work's `about` links each weigh
//! double their `mentions` counterpart.
//!
//! `owl:sameAs` reasoning is inert on this dataset: the generator asserts no
//! `sameAs` rels, so every `FILTER(!BOUND(?eq))` no-sameAs branch fires and the
//! sameAs branches contribute nothing. We score over the direct `about`/`mentions`
//! rels only.
//!
//! Two deliberate departures from the literal SPARQL, to produce the meaningful
//! per-work ranking the query name ("Creative Works *related* to a particular
//! one") and the brief ask for:
//!   * the four `count(*)` sub-selects never project their inner `?other_cw`, so
//!     read literally each `?cnt_*` is a single graph-wide scalar and every output
//!     row carries the *same* `?score`. We instead correlate each count with the
//!     specific other work, giving each work its own score.
//!   * `?other_creativeWork` has no `!= cwUri` filter, so the focal work — which
//!     shares every one of its own tags — would rank itself; we drop it, as the
//!     `?other_creativeWork` name intends.
//!
//! The inner `{ … ?other_creativeWork cwork:dateModified ?dt … } ORDER BY DESC(?dt)
//! LIMIT 10` is realized as the final ordering (score desc, then `dateModified`
//! desc) truncated to `limit`, with `dateModified` required on each candidate.

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::{has_label, node_by_uri};
use crate::props::PropExt;

/// The distinct entity targets of `work`'s outgoing `rel` (`about` / `mentions`).
fn targets(g: &GraphSnapshot, work: u32, rel: &str) -> HashSet<u32> {
    g.neighbors_by_type(work, Direction::Outgoing, rel)
        .collect()
}

/// Run SPB basic q9: creative works related to `cw_uri` by shared tagged
/// entities, scored `2·(about,about) + 1.5·(about,mentions) + 1·(mentions,about)
/// + 0.5·(mentions,mentions)`, returned as `(other_work_uri, score)` ordered by
/// score descending then `dateModified` descending, truncated to `limit` (the
/// template's `LIMIT 10`). The focal work and any candidate lacking a
/// `dateModified` are excluded; an unknown `cw_uri` yields an empty result.
pub fn run(g: &GraphSnapshot, cw_uri: &str, limit: usize) -> Vec<(String, f64)> {
    let Some(focal) = node_by_uri(g, cw_uri) else {
        return Vec::new();
    };
    let focal_about = targets(g, focal, "about");
    let focal_mentions = targets(g, focal, "mentions");

    // Candidate other works: those reaching a shared tagged entity through an
    // incoming about/mentions rel (cwork:tag = about ∪ mentions).
    let mut candidates: HashSet<u32> = HashSet::new();
    for &ent in focal_about.iter().chain(focal_mentions.iter()) {
        for rel in ["about", "mentions"] {
            for w in g.neighbors_by_type(ent, Direction::Incoming, rel) {
                if w != focal && has_label(g, w, "CreativeWork") {
                    candidates.insert(w);
                }
            }
        }
    }

    // (uri, dateModified, score) per qualifying candidate.
    // (node, dateModified, score): tally the shared-entity counts by testing each
    // candidate's about/mentions neighbours against the focal sets, rather than
    // materializing a HashSet per candidate; uris are resolved only for kept rows.
    let mut rows: Vec<(u32, &str, f64)> = Vec::new();
    for o in candidates {
        let Some(dt) = g.prop_str(o, "dateModified") else {
            continue;
        };
        let (mut a2a, mut m2a, mut a2m, mut m2m) = (0usize, 0usize, 0usize, 0usize);
        for e in g.neighbors_by_type(o, Direction::Outgoing, "about") {
            a2a += focal_about.contains(&e) as usize;
            m2a += focal_mentions.contains(&e) as usize;
        }
        for e in g.neighbors_by_type(o, Direction::Outgoing, "mentions") {
            a2m += focal_about.contains(&e) as usize;
            m2m += focal_mentions.contains(&e) as usize;
        }
        let score = 2.0 * a2a as f64 + 1.5 * a2m as f64 + 1.0 * m2a as f64 + 0.5 * m2m as f64;
        if score <= 0.0 {
            continue;
        }
        rows.push((o, dt, score));
    }

    // ORDER BY score DESC, then dateModified DESC (ISO-8601, hence lexicographic).
    rows.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(a.1))
    });
    rows.truncate(limit);
    rows.into_iter()
        .map(|(o, _dt, score)| (g.prop(o, "uri").str().unwrap_or("?").to_string(), score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Focal cwF tags entity A via `about` and entity B via `mentions`. Five other
    // works each share exactly one of those, exercising one factor apiece, plus a
    // duplicate top scorer (cw5) for the dateModified tie-break and an unrelated
    // work (cw6) sharing nothing.
    //   cw1 about A     -> (about, about)       = 2.0
    //   cw5 about A     -> (about, about)       = 2.0  (newer than cw1)
    //   cw2 mentions A  -> (about, mentions)    = 1.5
    //   cw3 about B     -> (mentions, about)    = 1.0
    //   cw4 mentions B  -> (mentions, mentions) = 0.5
    //   cw6 about Z     -> shares nothing       -> excluded
    const FIXTURE: &str = r#"
<http://ex/cwF> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cwF> <http://bbc/about> <http://ent/A> .
<http://ex/cwF> <http://bbc/mentions> <http://ent/B> .
<http://ex/cwF> <http://bbc/dateModified> "2024-06-01T00:00:00.000Z" .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/about> <http://ent/A> .
<http://ex/cw1> <http://bbc/dateModified> "2024-01-01T00:00:00.000Z" .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw5> <http://bbc/about> <http://ent/A> .
<http://ex/cw5> <http://bbc/dateModified> "2024-03-01T00:00:00.000Z" .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/mentions> <http://ent/A> .
<http://ex/cw2> <http://bbc/dateModified> "2024-01-02T00:00:00.000Z" .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/about> <http://ent/B> .
<http://ex/cw3> <http://bbc/dateModified> "2024-01-03T00:00:00.000Z" .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw4> <http://bbc/mentions> <http://ent/B> .
<http://ex/cw4> <http://bbc/dateModified> "2024-01-04T00:00:00.000Z" .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw6> <http://bbc/about> <http://ent/Z> .
<http://ex/cw6> <http://bbc/dateModified> "2024-01-05T00:00:00.000Z" .
"#;

    #[test]
    fn scores_and_orders_related_works_by_factor_then_recency() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, "http://ex/cwF", 100);
        // Distinct factors 2.0/1.5/1.0/0.5; cw5 outranks cw1 on the 2.0 tie by its
        // newer dateModified. cw6 (shares nothing) and the focal cwF are excluded.
        assert_eq!(
            rows,
            vec![
                ("http://ex/cw5".to_string(), 2.0),
                ("http://ex/cw1".to_string(), 2.0),
                ("http://ex/cw2".to_string(), 1.5),
                ("http://ex/cw3".to_string(), 1.0),
                ("http://ex/cw4".to_string(), 0.5),
            ]
        );
    }

    #[test]
    fn limit_truncates_the_ranking() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, "http://ex/cwF", 2);
        assert_eq!(
            rows,
            vec![
                ("http://ex/cw5".to_string(), 2.0),
                ("http://ex/cw1".to_string(), 2.0)
            ]
        );
    }

    #[test]
    fn unknown_focal_uri_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "http://ex/nope", 100).is_empty());
    }
}
