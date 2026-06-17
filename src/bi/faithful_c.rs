//! BI faithful queries Q3, Q4, Q10, Q15–Q17 and the simplified BI1/BI3 patterns.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::col_i64;
use super::faithful_b::person_by_plid;
use crate::props::*;

/// Q16 — Fake news detection. For two (tag, date) params, find people who made a
/// message with that tag on that date and have at most `max_knows` friends who
/// did the same; return people qualifying for BOTH, by combined message count.
/// Cypher: bi-16.cypher.
/// Q16 per-param: persons who made a message with `tag_name` on `day` and have
/// at most `max_knows` friends who did the same, with their message count.
pub(crate) fn q16_param_result(
    g: &GraphSnapshot,
    tag_name: &str,
    day: i64,
    max_knows: i64,
) -> HashMap<u32, i64> {
    let Some(tag) = tag_by_name(g, tag_name) else {
        return HashMap::new();
    };
    let mut cm: HashMap<u32, i64> = HashMap::new(); // person -> their tagged-on-day message count
    let mut creators_on_day: HashSet<u32> = HashSet::new();
    for msg in g.neighbors_by_type(tag, Direction::Incoming, &["hasTag"]) {
        if pi64(g, msg, "day") != day {
            continue;
        }
        for creator in g.neighbors_by_type(msg, Direction::Incoming, &["hasCreator"]) {
            *cm.entry(creator).or_insert(0) += 1;
            creators_on_day.insert(creator);
        }
    }
    cm.into_iter()
        .filter(|(p1, _)| {
            let cp2 = g
                .neighbors_by_type(*p1, Direction::Outgoing, &["knows"])
                .filter(|f| creators_on_day.contains(f))
                .count() as i64;
            cp2 <= max_knows
        })
        .collect()
}

pub(crate) fn q16_fake_news(
    g: &GraphSnapshot,
    ra: &HashMap<u32, i64>,
    rb: &HashMap<u32, i64>,
) -> Vec<(u32, i64, i64)> {
    let mut rows: Vec<(u32, i64, i64)> = ra
        .iter()
        .filter_map(|(&p, &ca)| rb.get(&p).map(|&cb| (p, ca, cb)))
        .collect();
    let plid_col = g.col("plid").map(|c| c.i64());
    rows.sort_by(|a, b| {
        (b.1 + b.2)
            .cmp(&(a.1 + a.2))
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(20);
    rows
}

/// Q10 — Experts in social circle. From `start` (by LDBC id), people at knows
/// shortest-distance in [min_dist, max_dist] who live in `country` and created
/// messages tagged with a tag of `tagclass`; count messages per (expert, tag).
/// Cypher: bi-10.cypher (start person/params adapted to SF1).
pub(crate) fn q10_experts(
    g: &GraphSnapshot,
    start_plid: i64,
    country_name: &str,
    tagclass_name: &str,
    min_dist: u32,
    max_dist: u32,
) -> Vec<(u32, String, i64)> {
    let Some(start) = person_by_plid(g, start_plid) else {
        return Vec::new();
    };
    // Shortest knows hop-distance from start, bounded to max_dist, via the core
    // bounded-BFS primitive.
    let dist = g.bfs_distances(start, Direction::Outgoing, &["knows"], Some(max_dist));
    let country = g.node_by_label_property("Country", "name", country_name);
    let tc = g.node_by_label_property("TagClass", "name", tagclass_name);
    let (Some(country), Some(tc)) = (country, tc) else {
        return Vec::new();
    };
    let mut in_country: HashSet<u32> = HashSet::new();
    for city in g.neighbors_by_type(country, Direction::Incoming, &["isPartOf"]) {
        for p in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            in_country.insert(p);
        }
    }
    let class_tags: HashSet<u32> = g
        .neighbors_by_type(tc, Direction::Incoming, &["hasType"])
        .collect();
    let mut counts: HashMap<(u32, u32), HashSet<u32>> = HashMap::new(); // (expert, tag) -> messages
    for (&expert, &d) in &dist {
        if d < min_dist || d > max_dist || !in_country.contains(&expert) {
            continue;
        }
        for msg in g.neighbors_by_type(expert, Direction::Outgoing, &["hasCreator"]) {
            let tags = g
                .neighbors_by_type(msg, Direction::Outgoing, &["hasTag"])
                .collect::<Vec<_>>();
            if tags.iter().any(|t| class_tags.contains(t)) {
                for &t in &tags {
                    counts.entry((expert, t)).or_default().insert(msg);
                }
            }
        }
    }
    let mut rows: Vec<(u32, String, i64)> = counts
        .into_iter()
        .map(|((e, t), msgs)| {
            (
                e,
                pstr(g, t, "name").unwrap_or("").to_string(),
                msgs.len() as i64,
            )
        })
        .collect();
    let plid_col = g.col("plid").map(|c| c.i64());
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(a.1.cmp(&b.1))
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

/// Q3 — Popular topics in a country. For forums whose moderator lives in
/// `country`, count distinct messages in the forums' post reply-trees that carry
/// a tag of `tagclass`; top 20 by count. Cypher: bi-3.cypher.
pub(crate) fn q3_popular_topics(
    g: &GraphSnapshot,
    country_name: &str,
    tagclass_name: &str,
) -> Vec<(i64, String, i64, i64, i64)> {
    let country = g.node_by_label_property("Country", "name", country_name);
    let tc = g.node_by_label_property("TagClass", "name", tagclass_name);
    let (Some(country), Some(tc)) = (country, tc) else {
        return Vec::new();
    };
    let class_tags: HashSet<u32> = g
        .neighbors_by_type(tc, Direction::Incoming, &["hasType"])
        .collect();
    let has_class_tag = |msg: u32| {
        g.neighbors_by_type(msg, Direction::Outgoing, &["hasTag"])
            .any(|t| class_tags.contains(&t))
    };
    let mut rows: Vec<(i64, String, i64, i64, i64)> = Vec::new();
    for city in g.neighbors_by_type(country, Direction::Incoming, &["isPartOf"]) {
        for person in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            for forum in g.neighbors_by_type(person, Direction::Incoming, &["hasModerator"]) {
                let mut msgs: HashSet<u32> = HashSet::new();
                for post in g.neighbors_by_type(forum, Direction::Outgoing, &["containerOf"]) {
                    let mut stack = vec![post];
                    let mut seen: HashSet<u32> = HashSet::new();
                    while let Some(n) = stack.pop() {
                        if !seen.insert(n) {
                            continue;
                        }
                        if has_class_tag(n) {
                            msgs.insert(n);
                        }
                        stack.extend(g.neighbors_by_type(n, Direction::Incoming, &["replyOf"]));
                    }
                }
                if !msgs.is_empty() {
                    rows.push((
                        pi64(g, forum, "flid"),
                        pstr(g, forum, "title").unwrap_or("").to_string(),
                        pi64(g, forum, "fday"),
                        pi64(g, person, "plid"),
                        msgs.len() as i64,
                    ));
                }
            }
        }
    }
    rows.sort_by(|a, b| b.4.cmp(&a.4).then(a.0.cmp(&b.0)));
    rows.truncate(20);
    rows
}

/// Q4 — Top message creators in a country. Take the top-100 forums (created
/// after `after_day`) by single-country membership, then rank their members by
/// the messages they created in those forums' post reply-trees. Returns
/// (person LDBC id, messageCount), top 100. Cypher: bi-4.cypher (name/date output
/// columns are deterministic from the id, so the cross-check uses id + count).
pub(crate) fn q4_top_creators(g: &GraphSnapshot, after_day: i64) -> (Vec<(i64, i64)>, Vec<i64>) {
    use hashbrown::{HashMap as FastMap, HashSet as FastSet};
    // Resolve the relationship types once and reuse them across the traversal.
    // The string-based, Vec-returning neighbors_by_type allocated a HashSet + Vec
    // on every call (~74% of this query's time); the zero-alloc *_of_type
    // accessors below match the type by an integer compare instead.
    let (Some(t_member), Some(t_loc), Some(t_part), Some(t_cont), Some(t_creator), Some(t_reply)) = (
        g.relationship_type_from_str("hasMember"),
        g.relationship_type_from_str("isLocatedIn"),
        g.relationship_type_from_str("isPartOf"),
        g.relationship_type_from_str("containerOf"),
        g.relationship_type_from_str("hasCreator"),
        g.relationship_type_from_str("replyOf"),
    ) else {
        return (Vec::new(), Vec::new());
    };
    let person_country = |p: u32| -> Option<u32> {
        let city = g.first_neighbor(p, Direction::Outgoing, t_loc)?;
        g.first_neighbor(city, Direction::Outgoing, t_part)
    };
    // Step 1: top-100 forums by (country, forum) member count.
    let mut cf: FastMap<(u32, u32), i64> = FastMap::new();
    if let Some(forums) = g.nodes_with_label("Forum") {
        for forum in forums.iter() {
            if pi64(g, forum, "fday") <= after_day {
                continue;
            }
            for m in g.neighbors_by_type(forum, Direction::Outgoing, t_member) {
                if let Some(country) = person_country(m) {
                    *cf.entry((country, forum)).or_insert(0) += 1;
                }
            }
        }
    }
    // Precompute (flid, lid) sort keys so the comparator does no property reads.
    let mut ranked: Vec<(i64, i64, i64, u32)> = cf
        .iter()
        .map(|(&(c, f), &n)| (n, pi64(g, f, "flid"), pi64(g, c, "lid"), f))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut top_forums: Vec<u32> = Vec::new();
    let mut seen_f: FastSet<u32> = FastSet::new();
    for (_, _, _, f) in ranked {
        if seen_f.insert(f) {
            top_forums.push(f);
            if top_forums.len() == 100 {
                break;
            }
        }
    }
    // Step 2: members of top forums, ranked by their messages in those forums.
    let mut members: FastSet<u32> = FastSet::new();
    for &f in &top_forums {
        for m in g.neighbors_by_type(f, Direction::Outgoing, t_member) {
            members.insert(m);
        }
    }
    // The replyOf graph is a forest (each thread roots at one Post), so a DFS down
    // a post's subtree never revisits — no `seen` set, and each message is counted
    // once (a plain per-creator counter, no inner dedup set). `stack` is reused.
    let mut msg_count: FastMap<u32, i64> = FastMap::new();
    let mut stack: Vec<u32> = Vec::new();
    for &f in &top_forums {
        for post in g.neighbors_by_type(f, Direction::Outgoing, t_cont) {
            stack.push(post);
        }
        while let Some(n) = stack.pop() {
            if let Some(creator) = g.first_neighbor(n, Direction::Incoming, t_creator) {
                if members.contains(&creator) {
                    *msg_count.entry(creator).or_insert(0) += 1;
                }
            }
            for c in g.neighbors_by_type(n, Direction::Incoming, t_reply) {
                stack.push(c);
            }
        }
    }
    let mut rows: Vec<(u32, i64)> = members
        .iter()
        .map(|&p| (p, msg_count.get(&p).copied().unwrap_or(0)))
        .collect();
    rows.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid")))
    });
    rows.truncate(100);
    let mut top_flids: Vec<i64> = top_forums.iter().map(|&f| pi64(g, f, "flid")).collect();
    top_flids.sort();
    (
        rows.into_iter()
            .map(|(p, c)| (pi64(g, p, "plid"), c))
            .collect(),
        top_flids,
    )
}

/// Q15 — Weighted interaction path. Weighted shortest path over the knows graph
/// where each edge weight is 1/(w+1); w sums reply interactions between the two
/// people whose thread root-post forum was created in [start_day, end_day]
/// (1.0 if a Post is involved, else 0.5). Returns the path cost, or -1 if
/// unreachable. Cypher: bi-15.cypher.
pub(crate) fn q15_weighted_path(
    g: &GraphSnapshot,
    p1: i64,
    p2: i64,
    start_day: i64,
    end_day: i64,
) -> f64 {
    use hashbrown::HashMap as FastMap;
    let (Some(src), Some(tgt)) = (person_by_plid(g, p1), person_by_plid(g, p2)) else {
        return -1.0;
    };
    let posts = g.nodes_with_label("Post");
    // w[(a,b)] over reply interactions whose thread's forum is in the window. Built
    // in parallel over comments (per-worker partial map + root-post memoization,
    // merged by par_fold); the contributions are exact 0.5/1.0 so the parallel sum
    // is deterministic. Relationship types are resolved once, not per call.
    let w: FastMap<(u32, u32), f64> = match (
        g.rel_type("replyOf"),
        g.rel_type("hasCreator"),
        g.rel_type("containerOf"),
        g.nodes_with_label("Comment"),
    ) {
        (Some(t_reply), Some(t_creator), Some(t_container), Some(comments)) => {
            let creator = |m: u32| g.first_neighbor(m, Direction::Incoming, t_creator);
            let is_post = |n: u32| posts.is_some_and(|p| p.contains(n));
            // Forest-root array for replyOf, built once; indexed lock-free in the
            // parallel fold below, so it replaces the per-worker root cache with no
            // per-node synchronization.
            let reply_roots = g.chain_roots(Direction::Outgoing, t_reply);
            comments.par_fold(
                FastMap::<(u32, u32), f64>::new,
                |mut acc, c| {
                    let Some(parent) = g.first_neighbor(c, Direction::Outgoing, t_reply) else {
                        return acc;
                    };
                    let (Some(cc), Some(pc)) = (creator(c), creator(parent)) else {
                        return acc;
                    };
                    if cc == pc {
                        return acc;
                    }
                    let root = reply_roots[c as usize];
                    let Some(forum) = g
                        .neighbors_by_type(root, Direction::Incoming, t_container)
                        .next()
                    else {
                        return acc;
                    };
                    let fday = pi64(g, forum, "fday");
                    if fday >= start_day && fday <= end_day {
                        let contrib = if is_post(parent) { 1.0 } else { 0.5 };
                        *acc.entry((cc.min(pc), cc.max(pc))).or_insert(0.0) += contrib;
                    }
                    acc
                },
                |mut a: FastMap<(u32, u32), f64>, b| {
                    for (k, v) in b {
                        *a.entry(k).or_insert(0.0) += v;
                    }
                    a
                },
            )
        }
        _ => FastMap::new(),
    };
    let sp = g.dijkstra(src, Direction::Both, "knows", Some(tgt), |from, rel| {
        let key = (from.min(rel.neighbor), from.max(rel.neighbor));
        1.0 / (w.get(&key).copied().unwrap_or(0.0) + 1.0)
    });
    sp.distance(tgt).filter(|d| d.is_finite()).unwrap_or(-1.0)
}

/// Q17 — Information propagation. For a tag, count distinct message2 per person1
/// where: person1's tagged message1 sits in forum1; a forum1 member (person2)
/// posted a tagged comment replying to message2 (by a different forum1 member
/// person3, also tagged) in a different forum2; message2 is >delta hours after
/// message1; and person1 is not a forum2 member. Top 10. Cypher: bi-17.cypher.
pub(crate) fn q17_information_propagation(
    g: &GraphSnapshot,
    tag_name: &str,
    delta_hours: i64,
) -> Vec<(i64, i64)> {
    let Some(tag) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let delta_ms = delta_hours * 3_600_000;
    let creator = |m: u32| {
        g.neighbors_by_type(m, Direction::Incoming, &["hasCreator"])
            .next()
    };
    // Forest-root array for replyOf, built once; the closure indexes it then
    // takes one containerOf hop to the forum.
    let reply_roots = g
        .rel_type("replyOf")
        .map(|rt| g.chain_roots(Direction::Outgoing, rt));
    let forum_of = |g: &GraphSnapshot, m: u32| -> Option<u32> {
        let root = match &reply_roots {
            Some(roots) => roots[m as usize],
            None => m,
        };
        g.neighbors_by_type(root, Direction::Incoming, &["containerOf"])
            .next()
    };
    let tagged: Vec<u32> = g
        .neighbors_by_type(tag, Direction::Incoming, &["hasTag"])
        .collect();
    let tagged_set: HashSet<u32> = tagged.iter().copied().collect();
    // message1 tuples (person1, forum1, ms1) and candidate (person2, person3, message2, forum2, ms2).
    let mut m1_list: Vec<(u32, u32, i64)> = Vec::new();
    let mut cand: Vec<(u32, u32, u32, u32, i64)> = Vec::new();
    for &m in &tagged {
        if let (Some(p1), Some(f1)) = (creator(m), forum_of(g, m)) {
            m1_list.push((p1, f1, pi64(g, m, "ms")));
        }
        if let Some(msg2) = g.first_neighbor(m, Direction::Outgoing, &["replyOf"]) {
            if tagged_set.contains(&msg2) {
                if let (Some(p2), Some(p3), Some(f2)) =
                    (creator(m), creator(msg2), forum_of(g, msg2))
                {
                    cand.push((p2, p3, msg2, f2, pi64(g, msg2, "ms")));
                }
            }
        }
    }
    let mut pm: HashMap<u32, HashSet<u32>> = HashMap::new();
    let ensure = |g: &GraphSnapshot, p: u32, pm: &mut HashMap<u32, HashSet<u32>>| {
        pm.entry(p).or_insert_with(|| {
            g.neighbors_by_type(p, Direction::Incoming, &["hasMember"])
                .collect()
        });
    };
    for &(p1, _, _) in &m1_list {
        ensure(g, p1, &mut pm);
    }
    for &(p2, p3, _, _, _) in &cand {
        ensure(g, p2, &mut pm);
        ensure(g, p3, &mut pm);
    }
    let mut counts: HashMap<u32, HashSet<u32>> = HashMap::new();
    for &(p2, p3, msg2, f2, ms2) in &cand {
        if p2 == p3 {
            continue;
        }
        let (fp2, fp3) = (&pm[&p2], &pm[&p3]);
        for &(p1, f1, ms1) in &m1_list {
            if f1 != f2
                && ms2 > ms1 + delta_ms
                && fp2.contains(&f1)
                && fp3.contains(&f1)
                && !pm[&p1].contains(&f2)
            {
                counts.entry(p1).or_default().insert(msg2);
            }
        }
    }
    let rows = counts
        .into_iter()
        .map(|(p, m)| (pi64(g, p, "plid"), m.len() as i64));
    top_k_by_key(rows, 10)
}

// ============ Simplified analytical patterns (synthetic-benchmark parity) ============

pub(crate) fn bi1_tag_evolution(g: &GraphSnapshot) -> usize {
    let mut pairs: HashMap<(u32, u32), u32> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let tags = g
                    .neighbors_by_type(msg, Direction::Outgoing, &["hasTag"])
                    .collect::<Vec<_>>();
                for i in 0..tags.len() {
                    for j in (i + 1)..tags.len() {
                        let pair = if tags[i] < tags[j] {
                            (tags[i], tags[j])
                        } else {
                            (tags[j], tags[i])
                        };
                        *pairs.entry(pair).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    pairs.len()
}

pub(crate) fn bi3_popular_topics(g: &GraphSnapshot) -> usize {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                for t in g.neighbors_by_type(msg, Direction::Outgoing, &["hasTag"]) {
                    *counts.entry(t).or_insert(0) += 1;
                }
            }
        }
    }
    counts.len()
}

pub(crate) fn top_creators(g: &GraphSnapshot, label: &str) -> usize {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    let persons = g.nodes_with_label("Person");
    if let Some(nodes) = g.nodes_with_label(label) {
        for msg in nodes.iter() {
            for creator in g.neighbors(msg, Direction::Incoming) {
                if persons.is_some_and(|p| p.contains(creator)) {
                    *counts.entry(creator).or_insert(0) += 1;
                }
            }
        }
    }
    counts.len()
}
