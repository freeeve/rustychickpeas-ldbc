//! BI faithful queries Q1, Q2, Q5–Q9, Q11, Q12.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot, ValueId};

use super::{col_i64, Q1Row};
use crate::props::*;

type Q1Groups = hashbrown::HashMap<(i64, bool, u8), (u64, i64)>;

pub(crate) fn q1_posting_summary(g: &GraphSnapshot, cutoff_day: i64) -> (Vec<Q1Row>, u64) {
    // Resolve each property's column once (hoisted out of the multi-million-row
    // scan). Each label is aggregated with NodeSet::par_fold — a parallel
    // hash-aggregate that mirrors Kùzu's threaded scan; the parallelism (and the
    // contiguous-range fast path) lives in core, so the query stays rayon-free.
    let col = |k: &str| g.property_key_from_str(k).and_then(|id| g.columns.get(&id));
    let (day_col, content_col, len_col, year_col) =
        (col("day"), col("content"), col("len"), col("year"));
    // Dense columns expose a typed slice: index it directly instead of building
    // a ValueId per cell. Falls back to the per-cell read for sparse/absent cols.
    let (day_s, len_s, year_s) = (
        day_col.and_then(|c| c.as_i64_slice()),
        len_col.and_then(|c| c.as_i64_slice()),
        year_col.and_then(|c| c.as_i64_slice()),
    );
    let content_s = content_col.and_then(|c| c.as_bool_slice());

    let mut groups: Q1Groups = Q1Groups::new();
    let mut total = 0u64;
    for (label, is_comment) in [("Post", false), ("Comment", true)] {
        let Some(nodes) = g.nodes_with_label(label) else {
            continue;
        };
        if nodes.is_empty() {
            continue;
        }
        // Per-message fold into a thread-local partial aggregate.
        let fold_one = |mut acc: (Q1Groups, u64), msg: u32| {
            let i = msg as usize;
            let day = match day_s {
                Some(s) => s[i],
                None => col_i64(day_col, msg),
            };
            if day >= cutoff_day {
                return acc;
            }
            acc.1 += 1;
            let has_content = match content_s {
                Some(s) => s[i],
                None => matches!(content_col.and_then(|c| c.get(msg)), Some(ValueId::Bool(true))),
            };
            if !has_content {
                return acc;
            }
            let len = match len_s {
                Some(s) => s[i],
                None => col_i64(len_col, msg),
            };
            let cat: u8 = if len < 40 {
                0
            } else if len < 80 {
                1
            } else if len < 160 {
                2
            } else {
                3
            };
            let year = match year_s {
                Some(s) => s[i],
                None => col_i64(year_col, msg),
            };
            let e = acc.0.entry((year, is_comment, cat)).or_insert((0, 0));
            e.0 += 1;
            e.1 += len;
            acc
        };
        let reduce_two = |mut a: (Q1Groups, u64), b: (Q1Groups, u64)| {
            for (k, (n, s)) in b.0 {
                let e = a.0.entry(k).or_insert((0, 0));
                e.0 += n;
                e.1 += s;
            }
            a.1 += b.1;
            a
        };
        let (part, sub_total) =
            nodes.par_fold(|| (Q1Groups::new(), 0u64), fold_one, reduce_two);
        for (k, (n, s)) in part {
            let e = groups.entry(k).or_insert((0, 0));
            e.0 += n;
            e.1 += s;
        }
        total += sub_total;
    }
    let mut rows: Vec<Q1Row> = groups
        .into_iter()
        .map(|((y, c, cat), (n, sum))| (y, c, cat, n, sum))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    (rows, total)
}

/// Q2 — Tag evolution. For tags of a given TagClass, count messages tagged with
/// them in two consecutive 100-day windows from `date0_day`; report the counts
/// and their absolute difference. Cypher: bi-2.cypher.
pub(crate) fn q2_tag_evolution(
    g: &GraphSnapshot,
    date0_day: i64,
    class_name: &str,
) -> Vec<(String, u64, u64, u64)> {
    // Target TagClass node.
    let target = g.nodes_with_label("TagClass").and_then(|tcs| {
        tcs.iter()
            .find(|&tc| pstr(g, tc, "name") == Some(class_name))
    });
    let Some(target) = target else {
        return Vec::new();
    };

    use hashbrown::{HashMap as FastMap, HashSet as FastSet};
    // Tags of that class.
    let mut qualifying: FastSet<u32> = FastSet::new();
    if let Some(tags) = g.nodes_with_label("Tag") {
        for t in tags.iter() {
            if g.neighbors_by_type(t, Direction::Outgoing, "hasType").any(|n| n == target) {
                qualifying.insert(t);
            }
        }
    }

    let (w1_lo, w1_hi) = (date0_day, date0_day + 100);
    let (w2_lo, w2_hi) = (date0_day + 100, date0_day + 200);
    // Resolve the day column + hasTag type once; the window filter scans every msg.
    // Index the dense slice directly, falling back to the per-cell read.
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let mut c1: FastMap<u32, u64> = FastMap::new();
    let mut c2: FastMap<u32, u64> = FastMap::new();
    if let Some(t_hastag) = g.rel_type("hasTag") {
        for label in ["Post", "Comment"] {
            let Some(nodes) = g.nodes_with_label(label) else {
                continue;
            };
            // Per message: window-classify, then count its qualifying tags into the
            // matching window's partial map; par_fold merges the workers.
            let (p1, p2) = nodes.par_fold(
                || (FastMap::<u32, u64>::new(), FastMap::<u32, u64>::new()),
                |mut acc, msg| {
                    let day = match day_s {
                        Some(s) => s[msg as usize],
                        None => col_i64(day_col, msg),
                    };
                    let in1 = w1_lo <= day && day < w1_hi;
                    let in2 = w2_lo <= day && day < w2_hi;
                    if !in1 && !in2 {
                        return acc;
                    }
                    for t in g.neighbors_by_type(msg, Direction::Outgoing, t_hastag) {
                        if qualifying.contains(&t) {
                            let m = if in1 { &mut acc.0 } else { &mut acc.1 };
                            *m.entry(t).or_insert(0) += 1;
                        }
                    }
                    acc
                },
                |mut a: (FastMap<u32, u64>, FastMap<u32, u64>), b| {
                    for (k, v) in b.0 {
                        *a.0.entry(k).or_insert(0) += v;
                    }
                    for (k, v) in b.1 {
                        *a.1.entry(k).or_insert(0) += v;
                    }
                    a
                },
            );
            for (k, v) in p1 {
                *c1.entry(k).or_insert(0) += v;
            }
            for (k, v) in p2 {
                *c2.entry(k).or_insert(0) += v;
            }
        }
    }

    let mut rows: Vec<(String, u64, u64, u64)> = qualifying
        .iter()
        .map(|&t| {
            let n1 = c1.get(&t).copied().unwrap_or(0);
            let n2 = c2.get(&t).copied().unwrap_or(0);
            let name = pstr(g, t, "name").unwrap_or("").to_string();
            (name, n1, n2, n1.abs_diff(n2))
        })
        .collect();
    rows.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q7 — Related topics. For a given tag, look at comments replying to messages
/// carrying that tag, and (for comments that do not themselves carry the tag)
/// count distinct such comments per *other* tag they carry. Cypher: bi-7.cypher.
pub(crate) fn q7_related_topics(g: &GraphSnapshot, tag_name: &str) -> Vec<(String, usize)> {
    let target = g
        .nodes_with_label("Tag")
        .and_then(|tags| tags.iter().find(|&t| pstr(g, t, "name") == Some(tag_name)));
    let Some(target) = target else {
        return Vec::new();
    };

    let mut related: HashMap<u32, HashSet<u32>> = HashMap::new();
    for msg in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        for comment in g.neighbors_by_type(msg, Direction::Incoming, &["replyOf"]) {
            let ctags: Vec<u32> = g.neighbors_by_type(comment, Direction::Outgoing, "hasTag").collect();
            if !ctags.contains(&target) {
                for &rt in &ctags {
                    related.entry(rt).or_default().insert(comment);
                }
            }
        }
    }
    let mut rows: Vec<(String, usize)> = related
        .into_iter()
        .map(|(rt, cs)| (pstr(g, rt, "name").unwrap_or("").to_string(), cs.len()))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q12 — How many people have a given number of messages. Per person, count
/// messages (with content, length < threshold, after a date) whose root Post's
/// language is in a given set; then histogram persons by that count (including
/// the zero bucket). Cypher: bi-12.cypher.
pub(crate) fn q12_message_counts(
    g: &GraphSnapshot,
    min_day: i64,
    len_thr: i64,
    langs: &[&str],
) -> Vec<(u64, u64)> {
    // Root-post language: the forest-root array maps each message to its thread's
    // terminal (root Post) via the functional `replyOf` chain; index it (built
    // once) and read the root's language. A Post has no `replyOf` parent, so its
    // terminal is itself.
    let reply_roots = g.rel_type("replyOf").map(|rt| g.chain_roots(Direction::Outgoing, rt));
    let root_lang = |start: u32| -> Option<&str> {
        let root = match &reply_roots {
            Some(roots) => roots[start as usize],
            None => start,
        };
        pstr(g, root, "lang")
    };

    // Read day/content/len from dense column slices (index by node id) instead of
    // re-resolving the property key on every one of millions of rows.
    let col = |k: &str| g.property_key_from_str(k).and_then(|id| g.columns.get(&id));
    let (day_col, content_col, len_col) = (col("day"), col("content"), col("len"));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let len_s = len_col.and_then(|c| c.as_i64_slice());
    let content_s = content_col.and_then(|c| c.as_bool_slice());

    let mut per_person: HashMap<u32, u64> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let i = msg as usize;
                let day = match day_s {
                    Some(s) => s[i],
                    None => pi64(g, msg, "day"),
                };
                if day <= min_day {
                    continue;
                }
                let content = match content_s {
                    Some(s) => s[i],
                    None => pbool(g, msg, "content"),
                };
                if !content {
                    continue;
                }
                let len = match len_s {
                    Some(s) => s[i],
                    None => pi64(g, msg, "len"),
                };
                if len >= len_thr {
                    continue;
                }
                if !matches!(root_lang(msg), Some(l) if langs.contains(&l)) {
                    continue;
                }
                for creator in g.neighbors_by_type(msg, Direction::Incoming, &["hasCreator"]) {
                    *per_person.entry(creator).or_insert(0) += 1;
                }
            }
        }
    }

    let total_persons = g.nodes_with_label("Person").map(|p| p.len()).unwrap_or(0) as u64;
    let mut hist: HashMap<u64, u64> = HashMap::new();
    for &c in per_person.values() {
        *hist.entry(c).or_insert(0) += 1;
    }
    hist.insert(0, total_persons.saturating_sub(per_person.len() as u64));
    let mut rows: Vec<(u64, u64)> = hist.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    rows
}

/// Q5 — Most active posters of a given topic. For a tag, score each creator of
/// tagged messages by 1*messages + 2*replies + 10*likes-received, top 100 by
/// score. Returns (person, messages, replies, likes, score). Cypher: bi-5.cypher.
pub(crate) fn q5_active_posters(g: &GraphSnapshot, tag_name: &str) -> Vec<(u32, u64, u64, u64, u64)> {
    let Some(target) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let mut agg: HashMap<u32, (u64, u64, u64)> = HashMap::new(); // person -> (msgs, replies, likes)
    for message in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        let likes = g.neighbors_by_type(message, Direction::Incoming, "likes").count() as u64;
        let replies = g.neighbors_by_type(message, Direction::Incoming, "replyOf").count() as u64;
        for person in g.neighbors_by_type(message, Direction::Incoming, &["hasCreator"]) {
            let e = agg.entry(person).or_insert((0, 0, 0));
            e.0 += 1;
            e.1 += replies;
            e.2 += likes;
        }
    }
    let mut rows: Vec<(u32, u64, u64, u64, u64)> = agg
        .into_iter()
        .map(|(p, (m, r, l))| (p, m, r, l, m + 2 * r + 10 * l))
        .collect();
    // Hoist the plid column once so the comparator indexes it instead of
    // re-resolving the property key on every comparison.
    let plid_col = g.property_key_from_str("plid").and_then(|id| g.columns.get(&id));
    rows.sort_by(|a, b| {
        b.4.cmp(&a.4)
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

/// Q6 — Most authoritative users on a topic. For each creator of tagged messages
/// (person1), find who liked those messages (person2), and sum the likes those
/// person2s received on their own messages. Cypher: bi-6.cypher.
pub(crate) fn q6_authoritative(g: &GraphSnapshot, tag_name: &str) -> Vec<(u32, u64)> {
    let Some(target) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let mut p1_to_p2: HashMap<u32, HashSet<u32>> = HashMap::new();
    for message1 in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        let likers: Vec<u32> = g.neighbors_by_type(message1, Direction::Incoming, "likes").collect();
        if likers.is_empty() {
            continue;
        }
        for person1 in g.neighbors_by_type(message1, Direction::Incoming, &["hasCreator"]) {
            p1_to_p2
                .entry(person1)
                .or_default()
                .extend(likers.iter().copied());
        }
    }
    let mut rows: Vec<(u32, u64)> = p1_to_p2
        .into_iter()
        .map(|(p1, p2set)| {
            let score: u64 = p2set
                .iter()
                .flat_map(|&p2| g.neighbors_by_type(p2, Direction::Outgoing, &["hasCreator"]))
                .map(|m2| g.neighbors_by_type(m2, Direction::Incoming, "likes").count() as u64)
                .sum();
            (p1, score)
        })
        .collect();
    let plid_col = g.property_key_from_str("plid").and_then(|id| g.columns.get(&id));
    rows.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

/// Q8 — Central Person for a Tag. Score persons by interest in the tag (×100) +
/// messages they made with the tag in a date window, then add their friends'
/// scores. Returns (person, score, friendsScore), top 100 by score+friendsScore.
/// Cypher: bi-8.cypher.
pub(crate) fn q8_central_person(
    g: &GraphSnapshot,
    tag_name: &str,
    start_day: i64,
    end_day: i64,
) -> Vec<(u32, i64, i64)> {
    let Some(tag) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let interested: HashSet<u32> = g
        .neighbors_by_type(tag, Direction::Incoming, &["hasInterest"])
        .into_iter()
        .collect();
    let mut msgcount: HashMap<u32, i64> = HashMap::new();
    for msg in g.neighbors_by_type(tag, Direction::Incoming, &["hasTag"]) {
        let day = pi64(g, msg, "day");
        if day > start_day && day < end_day {
            for creator in g.neighbors_by_type(msg, Direction::Incoming, &["hasCreator"]) {
                *msgcount.entry(creator).or_insert(0) += 1;
            }
        }
    }
    // Per-person base score (the same formula the friend-score uses).
    let mut score: HashMap<u32, i64> = HashMap::new();
    for &p in &interested {
        *score.entry(p).or_insert(0) += 100;
    }
    for (&p, &c) in &msgcount {
        *score.entry(p).or_insert(0) += c;
    }
    // friendsScore = sum of friends' base scores (non-candidates contribute 0).
    let mut rows: Vec<(u32, i64, i64)> = score
        .iter()
        .map(|(&p, &s)| {
            let fs: i64 = g
                .neighbors_by_type(p, Direction::Outgoing, &["knows"])
                .map(|f| score.get(&f).copied().unwrap_or(0))
                .sum();
            (p, s, fs)
        })
        .collect();
    let plid_col = g.property_key_from_str("plid").and_then(|id| g.columns.get(&id));
    rows.sort_by(|a, b| {
        (b.1 + b.2)
            .cmp(&(a.1 + a.2))
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

/// Q11 — Friend triangles. Count triangles in the `knows` graph among persons of
/// a given country where every edge was created within a date window. This is
/// the query that motivated the core `out_edges` API: it reads each knows edge's
/// `creationDate` (`kd`) during traversal via the edge's CSR position.
/// Cypher: bi-11.cypher.
pub(crate) fn q11_friend_triangles(
    g: &GraphSnapshot,
    country_name: &str,
    start_day: i64,
    end_day: i64,
) -> u64 {
    let country = g.nodes_with_label("Country").and_then(|cs| {
        cs.iter()
            .find(|&c| pstr(g, c, "name") == Some(country_name))
    });
    let Some(country) = country else {
        return 0;
    };
    // Persons located in a city of this country.
    let mut in_country: HashSet<u32> = HashSet::new();
    for city in g.neighbors_by_type(country, Direction::Incoming, &["isPartOf"]) {
        for p in g.neighbors_by_type(city, Direction::Incoming, &["isLocatedIn"]) {
            in_country.insert(p);
        }
    }
    // Date-filtered knows adjacency among in-country persons, reading each edge's
    // creationDate through its CSR position.
    // Hoist the edge `kd` (creationDate) column once; the traversal reads it for
    // every knows edge, so index it by CSR position instead of re-resolving the
    // property key per edge.
    let kd_col = g.property_key_from_str("kd").and_then(|id| g.rel_columns.get(&id));
    let mut adj: HashMap<u32, HashSet<u32>> = HashMap::new();
    for &a in &in_country {
        for e in g.relationships(a, Direction::Outgoing, &["knows"]) {
            if !in_country.contains(&e.neighbor) {
                continue;
            }
            let kd = match kd_col.and_then(|c| c.get(e.pos)) {
                Some(ValueId::I64(d)) => d,
                _ => continue,
            };
            if kd >= start_day && kd <= end_day {
                adj.entry(a).or_default().insert(e.neighbor);
            }
        }
    }
    // Count triangles a<b<c (by internal id) with all three edges present.
    let mut count: u64 = 0;
    for (&a, nbrs_a) in &adj {
        for &b in nbrs_a {
            if b <= a {
                continue;
            }
            if let Some(nbrs_b) = adj.get(&b) {
                for &c in nbrs_b {
                    if c > b && nbrs_a.contains(&c) {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

/// Q9 — Top thread initiators. For each person, count their posts in a date
/// window (threads) and the messages in those posts' reply trees, also in the
/// window. Cypher: bi-9.cypher.
pub(crate) fn q9_thread_initiators(g: &GraphSnapshot, start_day: i64, end_day: i64) -> Vec<(u32, u64, u64)> {
    let mut per_person: HashMap<u32, (u64, u64)> = HashMap::new(); // (threads, messages)
    // Hoist the day column once; the reply-tree DFS reads it for every visited
    // message, so index the dense slice instead of re-resolving the key per node.
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let day_at = |n: u32| -> i64 {
        match day_s {
            Some(s) => s[n as usize],
            None => col_i64(day_col, n),
        }
    };
    if let Some(posts) = g.nodes_with_label("Post") {
        for post in posts.iter() {
            let pd = day_at(post);
            if pd < start_day || pd > end_day {
                continue;
            }
            let Some(creator) = g.neighbors_by_type(post, Direction::Incoming, &["hasCreator"]).next() else {
                continue;
            };
            // Walk the post's reply tree; replies are created after their parent,
            // so prune any node past end_day (its whole subtree is later).
            let mut msgs = 0u64;
            let mut stack = vec![post];
            while let Some(n) = stack.pop() {
                let d = day_at(n);
                if d > end_day {
                    continue;
                }
                if d >= start_day {
                    msgs += 1;
                }
                stack.extend(g.neighbors_by_type(n, Direction::Incoming, &["replyOf"]));
            }
            let e = per_person.entry(creator).or_insert((0, 0));
            e.0 += 1;
            e.1 += msgs;
        }
    }
    let mut rows: Vec<(u32, u64, u64)> = per_person
        .into_iter()
        .map(|(p, (t, m))| (p, t, m))
        .collect();
    let plid_col = g.property_key_from_str("plid").and_then(|id| g.columns.get(&id));
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(col_i64(plid_col, a.0).cmp(&col_i64(plid_col, b.0)))
    });
    rows.truncate(100);
    rows
}

