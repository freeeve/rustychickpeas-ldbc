//! BI faithful queries Q13–Q14, Q18–Q20 and their location / study / interaction
//! helpers.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot, ValueId};

use super::col_i64;
use crate::props::*;

/// Q13 — Zombies in a country. Zombies are low-activity persons (created before
/// endDate with under one message per month). Score each by the share of likes
/// on their messages that come from other zombies. Cypher: bi-13.cypher.
pub(crate) fn q13_zombies(
    g: &GraphSnapshot,
    country_name: &str,
    end_day: i64,
    end_ym: i64,
) -> Vec<(u32, u64, u64)> {
    let country = g.node_by_label_property("Country", "name", country_name);
    let Some(country) = country else {
        return Vec::new();
    };
    let mut zombies: HashSet<u32> = HashSet::new();
    for city in g.neighbors_by_type(country, Direction::Incoming, &["isPartOf"]) {
        for p in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            if pi64(g, p, "pday") >= end_day {
                continue;
            }
            let mcount = g
                .neighbors_by_type(p, Direction::Outgoing, &["hasCreator"])
                .filter(|&m| pi64(g, m, "day") < end_day)
                .count() as i64;
            let months = end_ym - pi64(g, p, "pym") + 1;
            if months > 0 && mcount < months {
                zombies.insert(p);
            }
        }
    }
    let mut rows: Vec<(u32, u64, u64)> = zombies
        .iter()
        .map(|&z| {
            let mut zlc = 0u64;
            let mut tlc = 0u64;
            for m in g.neighbors_by_type(z, Direction::Outgoing, &["hasCreator"]) {
                for liker in g.neighbors_by_type(m, Direction::Incoming, &["likes"]) {
                    if pi64(g, liker, "pday") < end_day {
                        tlc += 1;
                    }
                    if zombies.contains(&liker) {
                        zlc += 1;
                    }
                }
            }
            (z, zlc, tlc)
        })
        .collect();
    let plid_col = g.i64_col("plid");
    rows.sort_by(|a, b| {
        let sa = if a.2 == 0 {
            0.0
        } else {
            a.1 as f64 / a.2 as f64
        };
        let sb = if b.2 == 0 {
            0.0
        } else {
            b.1 as f64 / b.2 as f64
        };
        sb.partial_cmp(&sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Tiebreak on the LDBC id (official "ORDER BY person.id"), so the
            // top-100 cut matches reference engines at like-ratio ties.
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

/// End-to-end validation of the core `bfs_distances` primitive on the real
/// `knows` graph: shortest-hop reachability from one person, returning (persons
/// reachable, eccentricity in hops). Not a faithful BI query — Q19/Q20 would need
/// a derived interaction-weight graph — but it exercises bounded BFS at SF scale.
pub(crate) fn knows_reachability(g: &GraphSnapshot) -> (usize, u32) {
    let persons: Vec<u32> = g
        .nodes_with_label("Person")
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    let Some(&source) = persons.first() else {
        return (0, 0);
    };
    let dist = g.bfs_distances(source, Direction::Both, &["knows"], None);
    let reachable = persons.iter().filter(|&&p| dist.contains_key(&p)).count();
    let ecc = persons
        .iter()
        .filter_map(|&p| dist.get(&p).copied())
        .max()
        .unwrap_or(0);
    (reachable, ecc)
}

/// Find a place node (City/Country/...) by its globally-unique LDBC id (`lid`),
/// via the cached label-free `node_by_property` index (one index serves City and
/// Country, replacing the per-label scan).
pub(crate) fn place_by_lid(g: &GraphSnapshot, lid: i64) -> Option<u32> {
    g.node_by_property("lid", lid)
}

/// Precompute the per-pair person interaction counts for Q19: the number of
/// reply interactions between the message creators of each (undirected) pair.
/// This is the weighted "projected graph" Q19 runs over; building it once
/// mirrors Q19's precomputation variant.
pub(crate) fn build_interaction_map(g: &GraphSnapshot) -> HashMap<(u32, u32), u32> {
    let mut interaction: HashMap<(u32, u32), u32> = HashMap::new();
    if let Some(comments) = g.nodes_with_label("Comment") {
        for c in comments.iter() {
            let Some(a) = g.first_neighbor(c, Direction::Incoming, &["hasCreator"]) else {
                continue;
            };
            for parent in g.neighbors_by_type(c, Direction::Outgoing, &["replyOf"]) {
                if let Some(b) = g.first_neighbor(parent, Direction::Incoming, &["hasCreator"]) {
                    if a != b {
                        *interaction.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    interaction
}

/// Q19 — Interaction path between cities. For people in city1 and city2, find the
/// shortest weighted path on the `knows` graph, where each edge weight is
/// 1/(reply interactions between the two people); return the 20 city1-city2 pairs
/// with the smallest path weight. Uses core `dijkstra` with a derived-weight
/// closure (the weight comes from the precomputed interaction map, not a stored
/// property). Cypher: bi-19.cypher.
pub(crate) fn q19_interaction_path(
    g: &GraphSnapshot,
    city1: u32,
    city2: u32,
    interaction: &HashMap<(u32, u32), u32>,
) -> Vec<(u32, u32, f64)> {
    let c2: HashSet<u32> = g
        .neighbors_by_type(city2, Direction::Incoming, "isLocatedIn")
        .collect();
    // Each city1 person's single-source search is independent: run them in
    // parallel and concat the partial result lists (sorted deterministically below).
    let mut results: Vec<(u32, u32, f64)> = g.par_neighbor_fold(
        city1,
        Direction::Incoming,
        "isLocatedIn",
        Vec::new,
        |mut acc, p1| {
            // Bidirectional search per (p1, p2): meets in the middle instead of
            // flooding the whole ~9.5k-node component to reach just a few targets.
            for &p2 in &c2 {
                if let Some(d) = g.weighted_shortest_path(
                    p1,
                    p2,
                    Direction::Both,
                    "knows",
                    |from, rel| {
                        match interaction.get(&(from.min(rel.neighbor), from.max(rel.neighbor))) {
                            Some(&n) if n > 0 => 1.0 / n as f64,
                            _ => f64::INFINITY, // know each other but never interacted
                        }
                    },
                ) {
                    acc.push((p1, p2, d));
                }
            }
            acc
        },
        |mut a, b| {
            a.extend(b);
            a
        },
    );
    let plid_col = g.i64_col("plid");
    results.sort_by(|a, b| {
        a.2.partial_cmp(&b.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Tiebreak on LDBC ids (official ORDER BY) so the top-20 cut matches Kùzu.
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
            .then(col_i64(plid_col, a.1).cmp(&col_i64(plid_col, b.1)))
    });
    results.truncate(20);
    results
}

/// Find an Organisation node (Company/University) by name.
pub(crate) fn org_by_name(g: &GraphSnapshot, label: &str, name: &str) -> Option<u32> {
    g.node_by_label_property(label, "name", name)
}

/// Find a Person node by its globally-unique LDBC id (`plid`), via the cached
/// label-free `node_by_property` index.
pub(crate) fn person_by_plid(g: &GraphSnapshot, plid: i64) -> Option<u32> {
    g.node_by_property("plid", plid)
}

/// Per-person study records (university, classYear), read from studyAt edges and
/// their classYear edge property.
pub(crate) fn build_studyat(g: &GraphSnapshot) -> HashMap<u32, Vec<(u32, i64)>> {
    let mut m: HashMap<u32, Vec<(u32, i64)>> = HashMap::new();
    // Hoist the edge `cy` (classYear) column once instead of resolving the key
    // per studyAt edge.
    let cy_col = g.property_key_from_str("cy").and_then(|id| g.rel_columns.get(&id));
    if let Some(persons) = g.nodes_with_label("Person") {
        for p in persons.iter() {
            let recs: Vec<(u32, i64)> = g
                .relationships(p, Direction::Outgoing, &["studyAt"])
                .map(|r| {
                    let cy = match cy_col.and_then(|c| c.get(r.pos)) {
                        Some(ValueId::I64(y)) => y,
                        _ => 0,
                    };
                    (r.neighbor, cy)
                })
                .collect();
            if !recs.is_empty() {
                m.insert(p, recs);
            }
        }
    }
    m
}

/// Q20 weight map: for knowing persons who studied at a common university, the
/// minimum |classYear difference| + 1 (smaller = closer cohort).
pub(crate) fn build_study_weight_map(
    g: &GraphSnapshot,
    studyat: &HashMap<u32, Vec<(u32, i64)>>,
) -> HashMap<(u32, u32), f64> {
    let mut wm: HashMap<(u32, u32), f64> = HashMap::new();
    for (&a, sa) in studyat {
        for b in g.neighbors_by_type(a, Direction::Outgoing, &["knows"]) {
            if b <= a {
                continue;
            }
            if let Some(sb) = studyat.get(&b) {
                let mut best: Option<i64> = None;
                for &(ua, ya) in sa {
                    for &(ub, yb) in sb {
                        if ua == ub {
                            best = Some(best.map_or((ya - yb).abs(), |x| x.min((ya - yb).abs())));
                        }
                    }
                }
                if let Some(d) = best {
                    wm.insert((a, b), (d + 1) as f64);
                }
            }
        }
    }
    wm
}

/// Q20 — Recruitment. From each employee of a company, the shortest weighted path
/// on the `knows` graph to a target person, where edge weight is the closeness of
/// the two people's university cohorts; return the 20 employees with the smallest
/// path weight. Uses core dijkstra (single-pair, with target early-exit) and a
/// derived-weight closure. Cypher: bi-20.cypher.
pub(crate) fn q20_recruitment(
    g: &GraphSnapshot,
    company: u32,
    person2: u32,
    weight_map: &HashMap<(u32, u32), f64>,
) -> Vec<(u32, f64)> {
    let mut results: Vec<(u32, f64)> = Vec::new();
    for p1 in g.neighbors_by_type(company, Direction::Incoming, &["workAt"]) {
        if p1 == person2 {
            continue;
        }
        let sp = g.dijkstra(
            p1,
            Direction::Both,
            &["knows"],
            Some(person2),
            |from, rel| {
                weight_map
                    .get(&(from.min(rel.neighbor), from.max(rel.neighbor)))
                    .copied()
                    .unwrap_or(f64::INFINITY)
            },
        );
        if let Some(d) = sp.distance(person2) {
            if d.is_finite() {
                results.push((p1, d));
            }
        }
    }
    let plid_col = g.i64_col("plid");
    results.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Tiebreak on the LDBC id (official ORDER BY) so the top-20 matches Kùzu.
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    results.truncate(20);
    results
}

/// Q18 — Friend recommendation. For people interested in a tag, count the mutual
/// friends shared with another (not-directly-known) person also interested in the
/// tag; top 20 ordered pairs by mutual-friend count. Cypher: bi-18.cypher.
pub(crate) fn q18_friend_recommendation(g: &GraphSnapshot, tag_name: &str) -> Vec<(u32, u32, u64)> {
    let Some(tag) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let interested: HashSet<u32> = g
        .neighbors_by_type(tag, Direction::Incoming, &["hasInterest"])
        .collect();
    // For each interested p1 and mutual friend m known by p1, each p2 known by m
    // who is interested, distinct from p1, and not directly known by p1.
    let mut mutual: HashMap<(u32, u32), HashSet<u32>> = HashMap::new();
    for &p1 in &interested {
        let p1_knows: HashSet<u32> = g
            .neighbors_by_type(p1, Direction::Outgoing, &["knows"])
            .collect();
        for &m in &p1_knows {
            for p2 in g.neighbors_by_type(m, Direction::Outgoing, &["knows"]) {
                if p2 != p1 && interested.contains(&p2) && !p1_knows.contains(&p2) {
                    mutual.entry((p1, p2)).or_default().insert(m);
                }
            }
        }
    }
    let mut rows: Vec<(u32, u32, u64)> = mutual
        .into_iter()
        .map(|((p1, p2), ms)| (p1, p2, ms.len() as u64))
        .collect();
    let plid_col = g.i64_col("plid");
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
            .then(col_i64(plid_col, a.1).cmp(&col_i64(plid_col, b.1)))
    });
    rows.truncate(20);
    rows
}

/// Q14 — International dialog. For each city of country1, the best-scoring
/// knows-pair (person1 in that city, person2 in country2) where score rewards
/// the presence of interaction types (4: p1 replied to p2; 1: p2 replied to p1;
/// 10: p1 likes p2's message; 1: p2 likes p1's). Cypher: bi-14.cypher.
pub(crate) fn q14_international_dialog(g: &GraphSnapshot, c1_name: &str, c2_name: &str) -> Vec<(u32, u32, String, i64)> {
    let country = |name: &str| g.node_by_label_property("Country", "name", name);
    let (Some(country1), Some(country2)) = (country(c1_name), country(c2_name)) else {
        return Vec::new();
    };
    // Hoist the plid column once for the candidate tiebreaks below.
    let plid_col = g.i64_col("plid");
    // persons whose message `p` replied to (via p's comments).
    let commented_on = |p: u32| -> HashSet<u32> {
        let mut s = HashSet::new();
        for msg in g.neighbors_by_type(p, Direction::Outgoing, &["hasCreator"]) {
            for parent in g.neighbors_by_type(msg, Direction::Outgoing, &["replyOf"]) {
                if let Some(cr) = g
                    .neighbors_by_type(parent, Direction::Incoming, &["hasCreator"])
                    .next()
                {
                    s.insert(cr);
                }
            }
        }
        s
    };
    // creators of messages `p` likes.
    let liked_creators = |p: u32| -> HashSet<u32> {
        let mut s = HashSet::new();
        for msg in g.neighbors_by_type(p, Direction::Outgoing, &["likes"]) {
            if let Some(cr) = g
                .neighbors_by_type(msg, Direction::Incoming, &["hasCreator"])
                .next()
            {
                s.insert(cr);
            }
        }
        s
    };
    // precompute interaction sets for country2 persons.
    let mut in_c2: HashSet<u32> = HashSet::new();
    let mut co_c2: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut lc_c2: HashMap<u32, HashSet<u32>> = HashMap::new();
    for city in g.neighbors_by_type(country2, Direction::Incoming, &["isPartOf"]) {
        for p in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            if in_c2.insert(p) {
                co_c2.insert(p, commented_on(p));
                lc_c2.insert(p, liked_creators(p));
            }
        }
    }
    let mut rows: Vec<(u32, u32, String, i64)> = Vec::new();
    for city in g.neighbors_by_type(country1, Direction::Incoming, &["isPartOf"]) {
        let city_name = pstr(g, city, "name").unwrap_or("").to_string();
        let mut best: Option<(i64, i64, i64, u32, u32)> = None; // score, p1plid, p2plid, p1, p2
        for p1 in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            let p1_co = commented_on(p1);
            let p1_lc = liked_creators(p1);
            for p2 in g.neighbors_by_type(p1, Direction::Outgoing, &["knows"]) {
                if !in_c2.contains(&p2) {
                    continue;
                }
                let mut score = 0i64;
                if p1_co.contains(&p2) {
                    score += 4;
                }
                if co_c2[&p2].contains(&p1) {
                    score += 1;
                }
                if p1_lc.contains(&p2) {
                    score += 10;
                }
                if lc_c2[&p2].contains(&p1) {
                    score += 1;
                }
                let (pa, pb) = (col_i64(plid_col, p1), col_i64(plid_col, p2));
                let cand = (score, pa, pb, p1, p2);
                best = Some(match best {
                    Some(b) if (b.0, -b.1, -b.2) >= (score, -pa, -pb) => b,
                    _ => cand,
                });
            }
        }
        if let Some((score, _, _, p1, p2)) = best {
            rows.push((p1, p2, city_name, score));
        }
    }
    rows.sort_by(|a, b| {
        b.3.cmp(&a.3)
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
            .then(col_i64(plid_col, a.1).cmp(&col_i64(plid_col, b.1)))
    });
    rows.truncate(100);
    rows
}

