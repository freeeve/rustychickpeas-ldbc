//! SNB Interactive (IC) workload: seed-anchored short/complex reads over the
//! same `initial_snapshot` the BI family loads (no extra download). Each query
//! is a hand-coded traversal reusing the loaded schema; the feasible tier
//! (IC1/2/9/13/14 + short reads IS1/2/3/5) is implemented here, the rest are
//! deferred (they need Forum-membership / tag-co-occurrence / organisation
//! expansions noted in tasks/003).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::{Column, Direction, GraphSnapshot, ValueId};

use crate::harness::{emit_json, jstr, time_query, Result};
use crate::loader::load_graph_opts;
use crate::props::*;

/// Reproducible IC substitution parameters derived from the loaded graph.
pub struct IcSeeds {
    /// Well-connected start person (max `knows` degree).
    pub person: u32,
    /// Upper creation-day bound that contains messages (for the "recent" reads).
    pub max_day: i64,
    /// A first name present among `person`'s <=3-hop friends (so IC1 is non-empty).
    pub first_name: String,
    /// A person reachable from `person` over `knows` (for IC13/IC14).
    pub person_b: u32,
}

/// Pick deterministic, reproducible IC seeds from the graph: the highest-degree
/// person (ties broken by smallest id), a fixed late date window, a first name
/// common among that person's neighbourhood, and the farthest person reachable
/// over `knows` (ties broken by smallest id).
pub fn pick_seeds(g: &GraphSnapshot) -> Option<IcSeeds> {
    let persons = g.nodes_with_label("Person")?;
    let mut best: Option<(u32, u32)> = None; // (degree, person)
    for p in persons.iter() {
        let deg = g.neighbors_by_type(p, Direction::Outgoing, "knows").count() as u32;
        best = Some(match best {
            Some((bd, bp)) if bd > deg || (bd == deg && bp <= p) => (bd, bp),
            _ => (deg, p),
        });
    }
    let person = best?.1;

    // A first name common in the start person's <=3-hop neighbourhood.
    let near = g.bfs_distances(person, Direction::Outgoing, "knows", Some(3));
    let mut name_counts: HashMap<String, u32> = HashMap::new();
    for (&p, &d) in &near {
        if d == 0 {
            continue;
        }
        if let Some(fname) = pstr(g, p, "fname") {
            *name_counts.entry(fname.to_string()).or_insert(0) += 1;
        }
    }
    let first_name = name_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(name, _)| name)?;

    // Farthest reachable person over `knows`, smallest id on ties.
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", None);
    let person_b = reach
        .iter()
        .filter(|(_, &d)| d >= 1)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&p, _)| p)?;

    Some(IcSeeds {
        person,
        max_day: days_from_civil(2013, 1, 1),
        first_name,
        person_b,
    })
}

/// IC1 — friends within 3 `knows` hops whose first name matches, ordered by
/// (distance, lastName, id). Returns (friend, distance, lastName), top 20.
pub fn ic1_friends_by_name(g: &GraphSnapshot, person: u32, first_name: &str) -> Vec<(u32, u32, String)> {
    let dist = g.bfs_distances(person, Direction::Outgoing, "knows", Some(3));
    let mut rows: Vec<(u32, u32, String)> = dist
        .iter()
        .filter(|(&p, &d)| d >= 1 && pstr(g, p, "fname") == Some(first_name))
        .map(|(&p, &d)| (p, d, pstr(g, p, "lname").unwrap_or("").to_string()))
        .collect();
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.2.cmp(&b.2))
            .then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid")))
    });
    rows.truncate(20);
    rows
}

/// IC2 — the 20 most recent messages by the seed's friends, created on/before
/// `max_day`, ordered by (creationDate desc, id). Returns (message, ms).
pub fn ic2_recent_messages(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    // Keep only the top 20 by (ms desc, id asc) rather than collecting every
    // friend-message and sorting the lot. The heap stores the result-ordering
    // key reversed, so it is a min-heap whose root is the worst kept message and
    // is evicted as soon as a better one arrives.
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(i64, Reverse<u32>)>> = BinaryHeap::with_capacity(21);
    for friend in g.neighbors_by_type(person, Direction::Outgoing, "knows") {
        for msg in g.neighbors_by_type(friend, Direction::Outgoing, "hasCreator") {
            if pi64(g, msg, "day") > max_day {
                continue;
            }
            let key = (pi64(g, msg, "ms"), Reverse(msg));
            if top.len() < 20 {
                top.push(Reverse(key));
            } else if key > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(key));
            }
        }
    }
    let mut rows: Vec<(u32, i64)> = top
        .into_iter()
        .map(|Reverse((ms, Reverse(id)))| (id, ms))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// IC9 — the 20 most recent messages by the seed's friends and friends-of-friends
/// (<=2 `knows` hops, excluding self), created on/before `max_day`.
pub fn ic9_fof_messages(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    // Hoist day/ms columns (no pi64 key re-resolution per message), and keep only
    // the top 20 by (ms desc, id asc) in a heap instead of collecting every FoF
    // message (millions) and sorting the lot.
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let ms_col = g.property_key_from_str("ms").and_then(|id| g.columns.get(&id));
    let ms_s = ms_col.and_then(|c| c.as_i64_slice());
    let day_of = |n: u32| -> i64 {
        match day_s {
            Some(s) => s[n as usize],
            None => match day_col.and_then(|c| c.get(n)) {
                Some(ValueId::I64(d)) => d,
                _ => 0,
            },
        }
    };
    let ms_of = |n: u32| -> i64 {
        match ms_s {
            Some(s) => s[n as usize],
            None => match ms_col.and_then(|c| c.get(n)) {
                Some(ValueId::I64(d)) => d,
                _ => 0,
            },
        }
    };
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(i64, Reverse<u32>)>> = BinaryHeap::with_capacity(21);
    for (&p, &d) in &reach {
        if d == 0 {
            continue;
        }
        for msg in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
            if day_of(msg) > max_day {
                continue;
            }
            let key = (ms_of(msg), Reverse(msg));
            if top.len() < 20 {
                top.push(Reverse(key));
            } else if key > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(key));
            }
        }
    }
    let mut rows: Vec<(u32, i64)> = top
        .into_iter()
        .map(|Reverse((ms, Reverse(id)))| (id, ms))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// IC13 — unweighted shortest-path length between two persons in the `knows`
/// graph (`-1` if unreachable). Built on the core `bfs_distances` primitive.
pub fn ic13_shortest_path(g: &GraphSnapshot, p1: u32, p2: u32) -> i64 {
    if p1 == p2 {
        return 0;
    }
    g.bfs_distances(p1, Direction::Outgoing, "knows", None)
        .get(&p2)
        .map(|&d| d as i64)
        .unwrap_or(-1)
}

/// IC14 — a weighted shortest path between two persons, where each `knows` edge
/// costs `1 / (interactions + 1)` (more reply interactions = cheaper). Returns
/// the path and its total cost, or `None` if unreachable.
pub fn ic14_weighted_path(
    g: &GraphSnapshot,
    p1: u32,
    p2: u32,
    interaction: &HashMap<(u32, u32), u32>,
) -> Option<f64> {
    // Bidirectional search (meets in the middle) — much faster than a single-source
    // dijkstra for this point-to-point query. knows is stored both directions, so
    // Outgoing forward + Incoming backward covers the graph without the redundant
    // double-traversal that Both would incur. Returns the path cost (the comparable
    // metric; Kùzu's WSHORTEST also reports cost, not an enumerated path).
    g.weighted_shortest_path(p1, p2, Direction::Outgoing, "knows", |from, rel| {
        let key = (from.min(rel.neighbor), from.max(rel.neighbor));
        1.0 / (interaction.get(&key).copied().unwrap_or(0) as f64 + 1.0)
    })
}

/// Reply interactions between message creators, keyed by the unordered person
/// pair — the IC14 edge-weight source (mirrors the BI Q19 projection).
pub fn build_knows_interaction(g: &GraphSnapshot) -> HashMap<(u32, u32), u32> {
    let mut m: HashMap<(u32, u32), u32> = HashMap::new();
    if let Some(comments) = g.nodes_with_label("Comment") {
        for c in comments.iter() {
            let Some(a) = g.neighbors_by_type(c, Direction::Incoming, "hasCreator").next() else {
                continue;
            };
            for parent in g.neighbors_by_type(c, Direction::Outgoing, "replyOf") {
                if let Some(b) = g.neighbors_by_type(parent, Direction::Incoming, "hasCreator").next() {
                    if a != b {
                        *m.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    m
}

/// IS1 — a person's profile: (firstName, lastName, creation day).
pub fn is1_profile(g: &GraphSnapshot, person: u32) -> Option<(String, String, i64)> {
    Some((
        pstr(g, person, "fname")?.to_string(),
        pstr(g, person, "lname")?.to_string(),
        pi64(g, person, "pday"),
    ))
}

/// IS2 — a person's own 10 most recent messages on/before `max_day`.
pub fn is2_recent_of_person(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    let mut rows: Vec<(u32, i64)> = g
        .neighbors_by_type(person, Direction::Outgoing, "hasCreator")
        .filter(|&m| pi64(g, m, "day") <= max_day)
        .map(|m| (m, pi64(g, m, "ms")))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(10);
    rows
}

/// IS3 — a person's direct `knows` friends (sorted by id).
pub fn is3_friends(g: &GraphSnapshot, person: u32) -> Vec<u32> {
    let mut f: Vec<u32> = g
        .neighbors_by_type(person, Direction::Outgoing, "knows")
        .collect();
    f.sort_unstable();
    f
}

/// IS5 — the creator of a message.
pub fn is5_message_creator(g: &GraphSnapshot, message: u32) -> Option<u32> {
    g.neighbors_by_type(message, Direction::Incoming, "hasCreator")
        .next()
}

/// IC4 — "new topics": Tags on the seed's friends' Posts created within
/// `[start_day, start_day + duration_days)` that were never on those friends'
/// Posts before `start_day`. Returns (tag, post_count), (count desc, name asc), top 10.
pub fn ic4_new_topics(g: &GraphSnapshot, person: u32, start_day: i64, duration_days: i64) -> Vec<(u32, u32)> {
    let end_day = start_day + duration_days;
    let posts = g.nodes_with_label("Post");
    // Hoist the day column (avoid pi64 re-resolving "day" per post).
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let day_of = |post: u32| -> i64 {
        match day_s {
            Some(s) => s[post as usize],
            None => match day_col.and_then(|c| c.get(post)) {
                Some(ValueId::I64(d)) => d,
                _ => 0,
            },
        }
    };

    // Tag ids form a contiguous block, so tally in-window tag uses in a dense
    // array indexed by (tag - lo) instead of a growing HashMap+HashSet; a BEFORE
    // sentinel marks tags also used before the window (excluded from "new").
    if let Some(range) = g.nodes_with_label("Tag").and_then(|t| t.as_range()) {
        const BEFORE: u32 = u32::MAX;
        let lo = range.start;
        let mut count = vec![0u32; (range.end - lo) as usize];
        for friend in g.neighbors_by_type(person, Direction::Outgoing, "knows") {
            for post in g.neighbors_by_type(friend, Direction::Outgoing, "hasCreator") {
                if !posts.is_some_and(|s| s.contains(post)) {
                    continue; // Posts only
                }
                let day = day_of(post);
                if day < start_day {
                    for t in g.neighbors_by_type(post, Direction::Outgoing, "hasTag") {
                        count[(t - lo) as usize] = BEFORE;
                    }
                } else if day < end_day {
                    for t in g.neighbors_by_type(post, Direction::Outgoing, "hasTag") {
                        let c = &mut count[(t - lo) as usize];
                        if *c != BEFORE {
                            *c += 1;
                        }
                    }
                }
            }
        }
        // Keep the top 10 by (count desc, name asc) in a size-10 min-heap instead
        // of collecting every qualifying tag and sorting — the heap key is the
        // result ordering reversed (root = worst kept), and names are borrowed
        // (no allocation). Tag names are unique, so the top-10 is unambiguous.
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let mut top: BinaryHeap<Reverse<(u32, Reverse<&str>, u32)>> = BinaryHeap::with_capacity(11);
        for i in 0..count.len() {
            let c = count[i];
            if c == 0 || c == BEFORE {
                continue;
            }
            let tag = lo + i as u32;
            let item = (c, Reverse(pstr(g, tag, "name").unwrap_or("")), tag);
            if top.len() < 10 {
                top.push(Reverse(item));
            } else if item > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(item));
            }
        }
        let mut rows: Vec<(u32, u32)> = top
            .into_iter()
            .map(|Reverse((c, _, tag))| (tag, c))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(pstr(g, a.0, "name").cmp(&pstr(g, b.0, "name"))));
        return rows;
    }

    // Fallback for non-contiguous Tag ids: growing HashMap/HashSet.
    let mut in_window: HashMap<u32, u32> = HashMap::new();
    let mut before: HashSet<u32> = HashSet::new();
    for friend in g.neighbors_by_type(person, Direction::Outgoing, "knows") {
        for post in g.neighbors_by_type(friend, Direction::Outgoing, "hasCreator") {
            if !posts.is_some_and(|s| s.contains(post)) {
                continue;
            }
            let day = day_of(post);
            if day < start_day {
                for t in g.neighbors_by_type(post, Direction::Outgoing, "hasTag") {
                    before.insert(t);
                }
            } else if day < end_day {
                for t in g.neighbors_by_type(post, Direction::Outgoing, "hasTag") {
                    *in_window.entry(t).or_insert(0) += 1;
                }
            }
        }
    }
    let mut rows: Vec<(u32, u32)> = in_window
        .into_iter()
        .filter(|(t, _)| !before.contains(t))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pstr(g, a.0, "name").cmp(&pstr(g, b.0, "name"))));
    rows.truncate(10);
    rows
}

/// IC6 — tag co-occurrence: among Posts created by the seed's friends/FoF
/// (<=2 `knows` hops, excluding self) tagged `tag_name`, count co-occurring other
/// Tags. Returns (other_tag, post_count), (count desc, name asc), top 10.
pub fn ic6_tag_cooccurrence(g: &GraphSnapshot, person: u32, tag_name: &str) -> Vec<(u32, u32)> {
    let Some(target) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let posts = g.nodes_with_label("Post");
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));

    // Dense co-occurrence counts over the contiguous Tag id range (like IC4), a
    // reusable per-post tag buffer (no Vec allocation per post), and a size-10
    // heap for the result instead of collect-all-then-sort.
    if let Some(range) = g.nodes_with_label("Tag").and_then(|t| t.as_range()) {
        let lo = range.start;
        let mut count = vec![0u32; (range.end - lo) as usize];
        let mut tags: Vec<u32> = Vec::new();
        for (&p, &d) in &reach {
            if d == 0 {
                continue;
            }
            for post in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
                if !posts.is_some_and(|s| s.contains(post)) {
                    continue; // Posts only
                }
                tags.clear();
                tags.extend(g.neighbors_by_type(post, Direction::Outgoing, "hasTag"));
                if !tags.contains(&target) {
                    continue; // post must carry the given tag
                }
                for &t in &tags {
                    if t != target {
                        count[(t - lo) as usize] += 1;
                    }
                }
            }
        }
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let mut top: BinaryHeap<Reverse<(u32, Reverse<&str>, u32)>> = BinaryHeap::with_capacity(11);
        for i in 0..count.len() {
            let c = count[i];
            if c == 0 {
                continue;
            }
            let tag = lo + i as u32;
            let item = (c, Reverse(pstr(g, tag, "name").unwrap_or("")), tag);
            if top.len() < 10 {
                top.push(Reverse(item));
            } else if item > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(item));
            }
        }
        let mut rows: Vec<(u32, u32)> = top
            .into_iter()
            .map(|Reverse((c, _, tag))| (tag, c))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(pstr(g, a.0, "name").cmp(&pstr(g, b.0, "name"))));
        return rows;
    }

    // Fallback for non-contiguous Tag ids: HashMap (still reuses the tag buffer).
    let mut counts: HashMap<u32, u32> = HashMap::new();
    let mut tags: Vec<u32> = Vec::new();
    for (&p, &d) in &reach {
        if d == 0 {
            continue;
        }
        for post in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
            if !posts.is_some_and(|s| s.contains(post)) {
                continue;
            }
            tags.clear();
            tags.extend(g.neighbors_by_type(post, Direction::Outgoing, "hasTag"));
            if !tags.contains(&target) {
                continue;
            }
            for &t in &tags {
                if t != target {
                    *counts.entry(t).or_insert(0) += 1;
                }
            }
        }
    }
    let mut rows: Vec<(u32, u32)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pstr(g, a.0, "name").cmp(&pstr(g, b.0, "name"))));
    rows.truncate(10);
    rows
}

/// IC8 — the 20 most recent replies to the start person's messages, ordered by
/// (replyCreationDate desc, reply id). Returns (reply, ms).
pub fn ic8_recent_replies(g: &GraphSnapshot, person: u32) -> Vec<(u32, i64)> {
    // Keep the top 20 by (ms desc, id asc) in a size-20 heap rather than collecting
    // every reply (a high-degree seed has thousands) and sorting the lot.
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(i64, Reverse<u32>)>> = BinaryHeap::with_capacity(21);
    for msg in g.neighbors_by_type(person, Direction::Outgoing, "hasCreator") {
        for reply in g.neighbors_by_type(msg, Direction::Incoming, "replyOf") {
            let key = (pi64(g, reply, "ms"), Reverse(reply));
            if top.len() < 20 {
                top.push(Reverse(key));
            } else if key > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(key));
            }
        }
    }
    let mut rows: Vec<(u32, i64)> = top
        .into_iter()
        .map(|Reverse((ms, Reverse(id)))| (id, ms))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// IS6 — (forum, moderator) for a message. `roots` is the `chain_roots` array
/// for `replyOf` (hoisted once by the caller); index it to get the message's
/// root Post, then one `containerOf` hop to the forum.
pub fn is6_forum_of_message(g: &GraphSnapshot, message: u32, roots: &[u32]) -> Option<(u32, u32)> {
    let root = *roots.get(message as usize)?; // root Post (a Post maps to itself)
    let forum = g.neighbors_by_type(root, Direction::Incoming, "containerOf").next()?;
    let moderator = g.neighbors_by_type(forum, Direction::Outgoing, "hasModerator").next()?;
    Some((forum, moderator))
}

/// IS7 — direct replies to a message: (reply, replyMs, replyAuthor, knows).
/// `knows` = replyAuthor is a `knows` friend of the original message's author
/// (false if the same person). Ordered (replyMs desc, replyAuthor plid asc).
pub fn is7_replies(g: &GraphSnapshot, message: u32) -> Vec<(u32, i64, u32, bool)> {
    let author = g.neighbors_by_type(message, Direction::Incoming, "hasCreator").next();
    let author_friends: HashSet<u32> = author
        .map(|a| g.neighbors_by_type(a, Direction::Outgoing, "knows").collect())
        .unwrap_or_default();
    let mut rows: Vec<(u32, i64, u32, bool)> = Vec::new();
    for reply in g.neighbors_by_type(message, Direction::Incoming, "replyOf") {
        let ra = g
            .neighbors_by_type(reply, Direction::Incoming, "hasCreator")
            .next()
            .unwrap_or(u32::MAX);
        let knows = author.is_some_and(|a| a != ra && author_friends.contains(&ra));
        rows.push((reply, pi64(g, reply, "ms"), ra, knows));
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pi64(g, a.2, "plid").cmp(&pi64(g, b.2, "plid"))));
    rows
}

/// IC3 — friends and friends-of-friends (<=2 `knows` hops, excluding self and
/// anyone whose home Country is X or Y) who created messages located in BOTH
/// Country X and Country Y within `[start_day, start_day + duration_days)`.
/// Returns (person, x_count, y_count), (x+y desc, plid asc), top 20.
pub fn ic3_friends_two_countries(g: &GraphSnapshot, person: u32, country_x: &str, country_y: &str, start_day: i64, duration_days: i64) -> Vec<(u32, u32, u32)> {
    let end_day = start_day + duration_days;
    let by_name = |name: &str| {
        g.nodes_with_label("Country")
            .and_then(|cs| cs.iter().find(|&c| pstr(g, c, "name") == Some(name)))
    };
    let (Some(cx), Some(cy)) = (by_name(country_x), by_name(country_y)) else {
        return Vec::new();
    };
    let home_country = |p: u32| -> Option<u32> {
        let city = g.neighbors_by_type(p, Direction::Outgoing, "isLocatedIn").next()?;
        g.neighbors_by_type(city, Direction::Outgoing, "isPartOf").next()
    };
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    // Hoist the day column once instead of pi64() re-resolving the "day" key
    // (an interner lookup) for every friend-of-friend message.
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    let day_s = day_col.and_then(|c| c.as_i64_slice());
    let day_of = |msg: u32| -> i64 {
        match day_s {
            Some(s) => s[msg as usize],
            None => match day_col.and_then(|c| c.get(msg)) {
                Some(ValueId::I64(d)) => d,
                _ => 0,
            },
        }
    };
    let mut rows: Vec<(u32, u32, u32)> = Vec::new();
    for (&p, &d) in &reach {
        if d == 0 || matches!(home_country(p), Some(c) if c == cx || c == cy) {
            continue;
        }
        let (mut xc, mut yc) = (0u32, 0u32);
        for msg in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
            let day = day_of(msg);
            if day < start_day || day >= end_day {
                continue;
            }
            match g.neighbors_by_type(msg, Direction::Outgoing, "msgCountry").next() {
                Some(c) if c == cx => xc += 1,
                Some(c) if c == cy => yc += 1,
                _ => {}
            }
        }
        if xc > 0 && yc > 0 {
            rows.push((p, xc, yc));
        }
    }
    rows.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid"))));
    rows.truncate(20);
    rows
}

/// IC5 — Forums the seed's friends/FoF (<=2 `knows` hops, excluding self) joined
/// after `min_day`, ranked by Posts in each Forum created by those post-`min_day`
/// members. Returns (forum, post_count), (count desc, flid asc), top 20.
pub fn ic5_new_groups(g: &GraphSnapshot, person: u32, min_day: i64) -> Vec<(u32, u32)> {
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    let hd_col = g.property_key_from_str("hd").and_then(|id| g.rel_columns.get(&id));
    // Member-centric: for each qualifying member, count only THEIR posts whose
    // container forum is one they joined after min_day. This touches only members'
    // posts, not every post of every forum (a post has one container Post-forum;
    // Comments have none, so only Posts count, matching the forum-post scan).
    let mut forum_counts: HashMap<u32, u32> = HashMap::new();
    // Reuse one set across FoFs (clear keeps capacity) rather than allocating a
    // fresh HashSet per member — that fresh-per-FoF alloc dominated IC5's bytes.
    let mut qforums: HashSet<u32> = HashSet::new();
    for (&p, &d) in &reach {
        if d == 0 {
            continue;
        }
        qforums.clear();
        for e in g.relationships(p, Direction::Incoming, &["hasMember"]) {
            if let Some(ValueId::I64(day)) = hd_col.and_then(|c| c.get(e.pos)) {
                if day > min_day {
                    qforums.insert(e.neighbor);
                }
            }
        }
        if qforums.is_empty() {
            continue;
        }
        for post in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
            if let Some(forum) = g.neighbors_by_type(post, Direction::Incoming, "containerOf").next() {
                if qforums.contains(&forum) {
                    *forum_counts.entry(forum).or_insert(0) += 1;
                }
            }
        }
    }
    let mut rows: Vec<(u32, u32)> = forum_counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pi64(g, a.0, "flid").cmp(&pi64(g, b.0, "flid"))));
    rows.truncate(20);
    rows
}

/// IC7 — the 20 most recent likers of the start person's messages, latest like
/// per liker, ordered (likeTime desc, liker plid asc). Returns
/// (liker, like_ms, message, is_new) where is_new = liker is not a `knows` friend.
pub fn ic7_recent_likers(g: &GraphSnapshot, person: u32) -> Vec<(u32, i64, u32, bool)> {
    let friends: HashSet<u32> = g.neighbors_by_type(person, Direction::Outgoing, "knows").collect();
    let ld_col = g.property_key_from_str("ld").and_then(|id| g.rel_columns.get(&id));
    let mut best: HashMap<u32, (i64, u32)> = HashMap::new();
    for msg in g.neighbors_by_type(person, Direction::Outgoing, "hasCreator") {
        for e in g.relationships(msg, Direction::Incoming, &["likes"]) {
            let lms = match ld_col.and_then(|c| c.get(e.pos)) {
                Some(ValueId::I64(v)) => v,
                _ => 0,
            };
            best.entry(e.neighbor)
                .and_modify(|cur| {
                    if lms > cur.0 || (lms == cur.0 && msg < cur.1) {
                        *cur = (lms, msg);
                    }
                })
                .or_insert((lms, msg));
        }
    }
    // Keep the top 20 by (likeDate desc, plid asc) in a size-20 heap instead of
    // sorting every liker — the full sort (a plid lookup per comparison) dominated.
    // plid is resolved once per liker here, not O(n log n) times.
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(i64, Reverse<i64>, u32, u32)>> = BinaryHeap::with_capacity(21);
    for (&liker, &(lms, msg)) in &best {
        let item = (lms, Reverse(pi64(g, liker, "plid")), liker, msg);
        if top.len() < 20 {
            top.push(Reverse(item));
        } else if item > top.peek().unwrap().0 {
            top.pop();
            top.push(Reverse(item));
        }
    }
    let mut rows: Vec<(u32, i64, u32, bool)> = top
        .into_iter()
        .map(|Reverse((lms, _, liker, msg))| (liker, lms, msg, !friends.contains(&liker)))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid"))));
    rows
}

/// IC10 — foaf (exactly 2 `knows` hops) born in [21st of `month` .. 22nd of the
/// next month), scored by (# their Posts tagged with a seed interest) − (# not).
/// Ordered (score desc, plid asc), top 10. Returns (foaf, score).
pub fn ic10_friend_recommend(g: &GraphSnapshot, person: u32, month: i64) -> Vec<(u32, i64)> {
    let next = month % 12 + 1;
    let interests: HashSet<u32> = g.neighbors_by_type(person, Direction::Outgoing, "hasInterest").collect();
    let posts = g.nodes_with_label("Post");
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    // Hoist the bmon/bdom (birthday filter, read per FoF) and plid (sort key)
    // columns so each read is a direct column lookup, not pi64 re-resolving the key.
    let col = |k: &str| g.property_key_from_str(k).and_then(|id| g.columns.get(&id));
    let (bmon_c, bdom_c, plid_c) = (col("bmon"), col("bdom"), col("plid"));
    let rd = |c: Option<&Column>, n: u32| -> i64 {
        match c.and_then(|col| col.get(n)) {
            Some(ValueId::I64(v)) => v,
            _ => 0,
        }
    };
    // Top 10 by (score desc, plid asc) in a heap; plid resolved once per FoF.
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(i64, Reverse<i64>, u32)>> = BinaryHeap::with_capacity(11);
    for (&foaf, &d) in &reach {
        if d != 2 {
            continue;
        }
        let (bmon, bdom) = (rd(bmon_c, foaf), rd(bdom_c, foaf));
        if !((bmon == month && bdom >= 21) || (bmon == next && bdom < 22)) {
            continue;
        }
        let (mut common, mut uncommon) = (0i64, 0i64);
        for msg in g.neighbors_by_type(foaf, Direction::Outgoing, "hasCreator") {
            if !posts.is_some_and(|p| p.contains(msg)) {
                continue;
            }
            if g.neighbors_by_type(msg, Direction::Outgoing, "hasTag").any(|t| interests.contains(&t)) {
                common += 1;
            } else {
                uncommon += 1;
            }
        }
        let item = (common - uncommon, Reverse(rd(plid_c, foaf)), foaf);
        if top.len() < 10 {
            top.push(Reverse(item));
        } else if item > top.peek().unwrap().0 {
            top.pop();
            top.push(Reverse(item));
        }
    }
    let mut rows: Vec<(u32, i64)> = top
        .into_iter()
        .map(|Reverse((score, _, foaf))| (foaf, score))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid"))));
    rows
}

/// IC11 — the seed's <=2-hop `knows` neighbourhood who worked (workFrom < `year`)
/// at a company in `country_name`. Ordered (workFrom asc, person plid asc,
/// company name desc), top 10. Returns (person, company, work_from).
pub fn ic11_job_referral(g: &GraphSnapshot, person: u32, country_name: &str, year: i64) -> Vec<(u32, u32, i64)> {
    let Some(country) = g
        .nodes_with_label("Country")
        .and_then(|cs| cs.iter().find(|&c| pstr(g, c, "name") == Some(country_name)))
    else {
        return Vec::new();
    };
    // Companies located in the country (orgPlace -> the country, or a city in it).
    let mut places_in_country: HashSet<u32> = HashSet::new();
    places_in_country.insert(country);
    for city in g.neighbors_by_type(country, Direction::Incoming, "isPartOf") {
        places_in_country.insert(city);
    }
    let mut in_country: HashSet<u32> = HashSet::new();
    if let Some(comps) = g.nodes_with_label("Company") {
        for org in comps.iter() {
            if g.neighbors_by_type(org, Direction::Outgoing, "orgPlace").any(|pl| places_in_country.contains(&pl)) {
                in_country.insert(org);
            }
        }
    }
    let wf_col = g.property_key_from_str("wf").and_then(|id| g.rel_columns.get(&id));
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    // Top 10 by (workFrom asc, plid asc, company name desc) in a heap; plid/name
    // resolved once per matching row, not per sort comparison. (The workAt scan
    // is the inherent CSR cost.)
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut top: BinaryHeap<Reverse<(Reverse<i64>, Reverse<i64>, &str, u32, u32)>> =
        BinaryHeap::with_capacity(11);
    for (&p, &d) in &reach {
        if d < 1 {
            continue;
        }
        for e in g.relationships(p, Direction::Outgoing, &["workAt"]) {
            if !in_country.contains(&e.neighbor) {
                continue;
            }
            let wf = match wf_col.and_then(|c| c.get(e.pos)) {
                Some(ValueId::I64(y)) => y,
                _ => continue,
            };
            if wf >= year {
                continue;
            }
            // larger key ranks earlier: smaller wf, smaller plid, larger name.
            let item = (
                Reverse(wf),
                Reverse(pi64(g, p, "plid")),
                pstr(g, e.neighbor, "name").unwrap_or(""),
                p,
                e.neighbor,
            );
            if top.len() < 10 {
                top.push(Reverse(item));
            } else if item > top.peek().unwrap().0 {
                top.pop();
                top.push(Reverse(item));
            }
        }
    }
    let mut rows: Vec<(u32, u32, i64)> = top
        .into_iter()
        .map(|Reverse((Reverse(wf), _, _, p, company))| (p, company, wf))
        .collect();
    rows.sort_by(|a, b| {
        a.2.cmp(&b.2)
            .then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid")))
            .then(pstr(g, b.1, "name").cmp(&pstr(g, a.1, "name")))
    });
    rows
}

/// IC12 — the seed's direct friends who replied (Comment -> replyOf -> Post) to
/// Posts tagged under `class_name` or a transitive subclass. Returns
/// (friend, reply_count, tag_names), (count desc, plid asc), top 20.
pub fn ic12_expert_search(g: &GraphSnapshot, person: u32, class_name: &str) -> Vec<(u32, usize, Vec<String>)> {
    let Some(root_class) = g
        .nodes_with_label("TagClass")
        .and_then(|cs| cs.iter().find(|&c| pstr(g, c, "name") == Some(class_name)))
    else {
        return Vec::new();
    };
    // The class plus all descendants (children point at the parent via isSubclassOf).
    let class_set: HashSet<u32> = g
        .bfs_distances(root_class, Direction::Incoming, "isSubclassOf", None)
        .into_keys()
        .collect();
    let qual_tag = |t: u32| {
        g.neighbors_by_type(t, Direction::Outgoing, "hasType").any(|c| class_set.contains(&c))
    };
    let posts = g.nodes_with_label("Post");
    // Collect qualifying tag *ids* per friend (dedup, no String allocation in the
    // hot loop); resolve the names only for the top-20 friends returned.
    let mut rows: Vec<(u32, usize, Vec<u32>)> = Vec::new();
    let mut tag_ids: HashSet<u32> = HashSet::new(); // reused across friends
    for friend in g.neighbors_by_type(person, Direction::Outgoing, "knows") {
        let mut count = 0usize;
        tag_ids.clear();
        for c in g.neighbors_by_type(friend, Direction::Outgoing, "hasCreator") {
            for parent in g.neighbors_by_type(c, Direction::Outgoing, "replyOf") {
                if !posts.is_some_and(|p| p.contains(parent)) {
                    continue;
                }
                let mut matched = false;
                for t in g.neighbors_by_type(parent, Direction::Outgoing, "hasTag") {
                    if qual_tag(t) {
                        matched = true;
                        tag_ids.insert(t);
                    }
                }
                if matched {
                    count += 1;
                }
            }
        }
        if count > 0 {
            rows.push((friend, count, tag_ids.iter().copied().collect()));
        }
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid"))));
    rows.truncate(20);
    rows.into_iter()
        .map(|(friend, count, tag_ids)| {
            let mut names: Vec<String> = tag_ids
                .iter()
                .filter_map(|&t| pstr(g, t, "name").map(str::to_string))
                .collect();
            names.sort();
            (friend, count, names)
        })
        .collect()
}

/// IS4 — a message's (creationMs, content). Needs the loader's `load_content`
/// option (the `ctext` property); image-only Posts fall back to their imageFile.
pub fn is4_message_content(g: &GraphSnapshot, message: u32) -> Option<(i64, String)> {
    Some((pi64(g, message, "ms"), pstr(g, message, "ctext")?.to_string()))
}

/// Load the snapshot, pick seeds, smoke-check each feasible IC query, then time
/// them (median of 5).
pub fn run() -> Result<()> {
    let default = PathBuf::from(
        "data/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot",
    );
    // Optional flags: --only <id> (e.g. ic5), --repeat <n>, --alloc. The first
    // non-flag arg stays the snapshot path. See harness::BenchCfg.
    let (mut only, mut runs_override, mut alloc, mut positional) =
        (None, 0usize, false, None::<String>);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--only" => only = it.next().map(|s| s.to_lowercase()),
            "--repeat" => runs_override = it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            "--alloc" => alloc = true,
            s if !s.starts_with("--") && positional.is_none() => positional = Some(s.to_string()),
            _ => {}
        }
    }
    crate::harness::set_bench_cfg(crate::harness::BenchCfg {
        only,
        runs: runs_override,
        alloc,
    });
    let snapshot = positional.map(PathBuf::from).unwrap_or(default);
    if !snapshot.join("dynamic").is_dir() {
        return Err(format!(
            "no 'dynamic' dir under {}; pass the initial_snapshot path as arg 1",
            snapshot.display()
        )
        .into());
    }

    eprintln!("Loading LDBC graph from {} ...", snapshot.display());
    let t = Instant::now();
    let (graph, s) = load_graph_opts(&snapshot, true)?; // IS4 needs message content text
    println!("\n=== LDBC SNB Interactive — SF1 (real data) ===");
    println!(
        "Loaded {} persons, {} posts, {} comments in {:.1}s\n",
        s.persons,
        s.posts,
        s.comments,
        t.elapsed().as_secs_f64()
    );

    let seeds = pick_seeds(&graph).ok_or("could not pick IC seeds (no persons?)")?;
    println!(
        "Seeds: person={} (plid {}), person_b={} (plid {}), firstName=\"{}\", maxDay={}",
        seeds.person,
        pi64(&graph, seeds.person, "plid"),
        seeds.person_b,
        pi64(&graph, seeds.person_b, "plid"),
        seeds.first_name,
        seeds.max_day
    );

    // Derived params shared by the cross-check emit and the smoke/timing runs.
    // IC6 needs a tag the neighbourhood actually posts; derive its most common one.
    let (ic4_start, ic4_dur): (i64, i64) = (days_from_civil(2011, 1, 1), 365);
    let seed_tag_name = {
        let mut c: HashMap<u32, u32> = HashMap::new();
        for f in graph.neighbors_by_type(seeds.person, Direction::Outgoing, "knows") {
            for post in graph.neighbors_by_type(f, Direction::Outgoing, "hasCreator") {
                for t in graph.neighbors_by_type(post, Direction::Outgoing, "hasTag") {
                    *c.entry(t).or_insert(0) += 1;
                }
            }
        }
        c.into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
            .and_then(|(t, _)| pstr(&graph, t, "name"))
            .unwrap_or("")
            .to_string()
    };
    // IS6/IS7 anchor on the seed's newest Post (both engines derive it the same way).
    let posts = graph.nodes_with_label("Post");
    let seed_post = graph
        .neighbors_by_type(seeds.person, Direction::Outgoing, "hasCreator")
        .filter(|&m| posts.is_some_and(|p| p.contains(m)))
        .max_by_key(|&m| pi64(&graph, m, "ms"));
    let reply_roots = graph
        .rel_type("replyOf")
        .map(|rt| graph.chain_roots(Direction::Outgoing, rt));
    // IC11/IC12 params: the seed's home country and the seed_tag's TagClass.
    let seed_country = graph
        .neighbors_by_type(seeds.person, Direction::Outgoing, "isLocatedIn")
        .next()
        .and_then(|city| graph.neighbors_by_type(city, Direction::Outgoing, "isPartOf").next())
        .and_then(|country| pstr(&graph, country, "name"))
        .unwrap_or("India")
        .to_string();
    let seed_class_name = tag_by_name(&graph, &seed_tag_name)
        .and_then(|t| graph.neighbors_by_type(t, Direction::Outgoing, "hasType").next())
        .and_then(|c| pstr(&graph, c, "name"))
        .unwrap_or("")
        .to_string();

    // Cross-check emit: dump comparable projections (LDBC ids / ms timestamps /
    // tag names, not internal node ids) so `kuzu/run_ic.py` can diff against Kùzu
    // via the shared `compare.py`. Emit mode skips the timing block.
    if let Ok(dir) = std::env::var("LDBC_EMIT_JSON") {
        let plid = |p: u32| pi64(&graph, p, "plid");
        let arr_ms = |rows: &[(u32, i64)]| {
            format!(
                "[{}]",
                rows.iter()
                    .map(|(_, ms)| format!("[{ms}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let is2 = is2_recent_of_person(&graph, seeds.person, seeds.max_day);
        let is5 = is2.first().and_then(|&(m, _)| is5_message_creator(&graph, m));
        let mut friends: Vec<i64> = is3_friends(&graph, seeds.person)
            .iter()
            .map(|&f| plid(f))
            .collect();
        friends.sort_unstable();
        emit_json(
            &dir,
            "seeds.json",
            format!(
                "{{\"person\":{},\"person_b\":{},\"first_name\":{},\"max_day\":{},\"seed_tag\":{},\"ic4_start\":{},\"ic4_dur\":{},\"seed_country\":{},\"seed_class\":{}}}",
                plid(seeds.person),
                plid(seeds.person_b),
                jstr(&seeds.first_name),
                seeds.max_day,
                jstr(&seed_tag_name),
                ic4_start,
                ic4_dur,
                jstr(&seed_country),
                jstr(&seed_class_name)
            ),
        );
        emit_json(
            &dir,
            "ic2.rust.json",
            arr_ms(&ic2_recent_messages(&graph, seeds.person, seeds.max_day)),
        );
        emit_json(
            &dir,
            "ic9.rust.json",
            arr_ms(&ic9_fof_messages(&graph, seeds.person, seeds.max_day)),
        );
        emit_json(
            &dir,
            "ic13.rust.json",
            format!("[[{}]]", ic13_shortest_path(&graph, seeds.person, seeds.person_b)),
        );
        emit_json(&dir, "is2.rust.json", arr_ms(&is2));
        emit_json(
            &dir,
            "is3.rust.json",
            format!(
                "[{}]",
                friends
                    .iter()
                    .map(|p| format!("[{p}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        emit_json(&dir, "is5.rust.json", format!("[[{}]]", is5.map(plid).unwrap_or(-1)));

        // Schema-compatible tier (task 052): IC4/IC6 -> [tagName, count],
        // IC8 -> [ms], IS6 -> [forumFlid, moderatorPlid], IS7 -> [ms, authorPlid, knows].
        let arr_tag = |rows: &[(u32, u32)]| {
            format!(
                "[{}]",
                rows.iter()
                    .map(|(t, c)| format!("[{},{c}]", jstr(pstr(&graph, *t, "name").unwrap_or(""))))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        emit_json(&dir, "ic4.rust.json", arr_tag(&ic4_new_topics(&graph, seeds.person, ic4_start, ic4_dur)));
        emit_json(&dir, "ic6.rust.json", arr_tag(&ic6_tag_cooccurrence(&graph, seeds.person, &seed_tag_name)));
        emit_json(&dir, "ic8.rust.json", arr_ms(&ic8_recent_replies(&graph, seeds.person)));
        let is6 = seed_post.and_then(|m| is6_forum_of_message(&graph, m, reply_roots.as_deref().unwrap_or(&[])));
        emit_json(
            &dir,
            "is6.rust.json",
            match is6 {
                Some((f, mo)) => format!("[[{},{}]]", pi64(&graph, f, "flid"), plid(mo)),
                None => "[]".to_string(),
            },
        );
        let is7 = seed_post.map(|m| is7_replies(&graph, m)).unwrap_or_default();
        emit_json(
            &dir,
            "is7.rust.json",
            format!(
                "[{}]",
                is7.iter()
                    .map(|(_, ms, a, k)| format!("[{ms},{},{}]", plid(*a), *k as i32))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );

        // Loader-backed tier (task 053): IC1 -> [dist, lastName, plid],
        // IC3 -> [plid, x, y], IC5 -> [forumFlid, count], IC7 -> [ms, likerPlid, isNew],
        // IC10 -> [plid, score], IC11 -> [plid, companyName, workFrom],
        // IC12 -> [plid, count], IS1 -> [firstName, lastName, pday].
        let join = |v: Vec<String>| format!("[{}]", v.join(","));
        let ic1 = ic1_friends_by_name(&graph, seeds.person, &seeds.first_name);
        emit_json(&dir, "ic1.rust.json", join(ic1.iter().map(|(p, d, ln)| format!("[{d},{},{}]", jstr(ln), plid(*p))).collect()));
        let ic3 = ic3_friends_two_countries(&graph, seeds.person, "China", "Germany", days_from_civil(2010, 1, 1), 1500);
        emit_json(&dir, "ic3.rust.json", join(ic3.iter().map(|(p, x, y)| format!("[{},{x},{y}]", plid(*p))).collect()));
        let ic5 = ic5_new_groups(&graph, seeds.person, days_from_civil(2011, 1, 1));
        emit_json(&dir, "ic5.rust.json", join(ic5.iter().map(|(f, c)| format!("[{},{c}]", pi64(&graph, *f, "flid"))).collect()));
        let ic7 = ic7_recent_likers(&graph, seeds.person);
        emit_json(&dir, "ic7.rust.json", join(ic7.iter().map(|(l, ms, _, new)| format!("[{ms},{},{}]", plid(*l), *new as i32)).collect()));
        let ic10 = ic10_friend_recommend(&graph, seeds.person, 1);
        emit_json(&dir, "ic10.rust.json", join(ic10.iter().map(|(p, s)| format!("[{},{s}]", plid(*p))).collect()));
        let ic11 = ic11_job_referral(&graph, seeds.person, &seed_country, 2030);
        emit_json(&dir, "ic11.rust.json", join(ic11.iter().map(|(p, co, wf)| format!("[{},{},{wf}]", plid(*p), jstr(pstr(&graph, *co, "name").unwrap_or("")))).collect()));
        let ic12 = ic12_expert_search(&graph, seeds.person, &seed_class_name);
        emit_json(&dir, "ic12.rust.json", join(ic12.iter().map(|(f, c, _)| format!("[{},{c}]", plid(*f))).collect()));
        emit_json(&dir, "is1.rust.json", match is1_profile(&graph, seeds.person) {
            Some((fname, lname, _pday)) => format!("[[{},{}]]", jstr(&fname), jstr(&lname)),
            None => "[]".to_string(),
        });
        // IC14: weighted shortest-path cost (path node ids aren't comparable
        // across engines, so compare the cost rounded to 6 dp). (task 054)
        let interaction = build_knows_interaction(&graph);
        emit_json(&dir, "ic14.rust.json", match ic14_weighted_path(&graph, seeds.person, seeds.person_b, &interaction) {
            Some(c) => format!("[[{c:.6}]]"),
            None => "[]".to_string(),
        });
        println!("emitted IC cross-check JSON (ic1-14, is1-7 sans is4) to {dir}");
        return Ok(());
    }

    // Smoke checks (shape, mirroring the BI "surfaces real data" assertions).
    let ic1 = ic1_friends_by_name(&graph, seeds.person, &seeds.first_name);
    let ic2 = ic2_recent_messages(&graph, seeds.person, seeds.max_day);
    let ic9 = ic9_fof_messages(&graph, seeds.person, seeds.max_day);
    let ic13 = ic13_shortest_path(&graph, seeds.person, seeds.person_b);
    let interaction = build_knows_interaction(&graph);
    let ic14 = ic14_weighted_path(&graph, seeds.person, seeds.person_b, &interaction);
    assert!(!ic1.is_empty(), "IC1 returned no friends for the seed name");
    assert!(!ic2.is_empty(), "IC2 returned no recent messages");
    assert!(!ic9.is_empty(), "IC9 returned no FoF messages");
    assert!(ic13 >= 1, "IC13 path length should be >= 1 for a reachable pair");
    assert!(ic14.is_some(), "IC14 found no weighted path");
    println!(
        "  IC1 rows: {}; IC2 recent: {}; IC9 FoF: {}; IC13 hops: {}; IC14 cost: {:.4}",
        ic1.len(),
        ic2.len(),
        ic9.len(),
        ic13,
        ic14.unwrap_or(-1.0)
    );

    // Deferred-tier queries enabled with no loader change: IC4/IC6/IC8, IS6/IS7.
    let ic4 = ic4_new_topics(&graph, seeds.person, ic4_start, ic4_dur);
    let ic6 = ic6_tag_cooccurrence(&graph, seeds.person, &seed_tag_name);
    let ic8 = ic8_recent_replies(&graph, seeds.person);
    assert!(ic4.len() <= 10, "IC4 over-returned");
    assert!(ic6.len() <= 10, "IC6 over-returned");
    let (is6_ok, is7_n) = match seed_post {
        Some(msg) => (
            is6_forum_of_message(&graph, msg, reply_roots.as_deref().unwrap_or(&[])).is_some(),
            is7_replies(&graph, msg).len(),
        ),
        None => (false, 0),
    };
    println!(
        "  IC4 new-topics: {}; IC6 co-tags(\"{}\"): {}; IC8 replies: {}; IS6 forum: {}; IS7 replies: {}",
        ic4.len(),
        seed_tag_name,
        ic6.len(),
        ic8.len(),
        is6_ok,
        is7_n
    );

    // Loader-backed queries (IC3/IC5/IC7/IC10/IC11/IC12); country/class params
    // (seed_country, seed_class_name) are derived above and shared with the emit.
    let ic3 = ic3_friends_two_countries(&graph, seeds.person, "China", "Germany", days_from_civil(2010, 1, 1), 1500);
    let ic5 = ic5_new_groups(&graph, seeds.person, days_from_civil(2011, 1, 1));
    let ic7 = ic7_recent_likers(&graph, seeds.person);
    let ic10 = ic10_friend_recommend(&graph, seeds.person, 1);
    let ic11 = ic11_job_referral(&graph, seeds.person, &seed_country, 2030);
    let ic12 = ic12_expert_search(&graph, seeds.person, &seed_class_name);
    assert!(ic3.len() <= 20 && ic5.len() <= 20 && ic7.len() <= 20);
    assert!(ic10.len() <= 10 && ic11.len() <= 10 && ic12.len() <= 20);
    println!(
        "  IC3 two-countries: {}; IC5 new-groups: {}; IC7 likers: {}; IC10 recommend: {}; IC11 referral(\"{}\"): {}; IC12 experts(\"{}\"): {}",
        ic3.len(),
        ic5.len(),
        ic7.len(),
        ic10.len(),
        seed_country,
        ic11.len(),
        seed_class_name,
        ic12.len()
    );
    let is4 = seed_post.and_then(|m| is4_message_content(&graph, m));
    assert!(is4.is_some(), "IS4 content not loaded (load_content off?)");
    if let Some((ms, text)) = &is4 {
        println!("  IS4 content: ms={ms}, {} chars", text.chars().count());
    }

    let runs = 5;
    println!("\nTimings (median of {runs}):");
    time_query("IC1 friends-by-name", runs, || {
        ic1_friends_by_name(&graph, seeds.person, &seeds.first_name).len()
    });
    time_query("IC2 recent friend messages", runs, || {
        ic2_recent_messages(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IC9 recent FoF messages", runs, || {
        ic9_fof_messages(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IC13 unweighted shortest path", runs, || {
        ic13_shortest_path(&graph, seeds.person, seeds.person_b).max(0) as usize
    });
    time_query("IC14 weighted shortest path", runs, || {
        ic14_weighted_path(&graph, seeds.person, seeds.person_b, &interaction).is_some() as usize
    });
    time_query("IS1 person profile", runs, || {
        is1_profile(&graph, seeds.person).is_some() as usize
    });
    time_query("IS2 person recent messages", runs, || {
        is2_recent_of_person(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IS3 person friends", runs, || {
        is3_friends(&graph, seeds.person).len()
    });
    time_query("IC4 new topics", runs, || {
        ic4_new_topics(&graph, seeds.person, ic4_start, ic4_dur).len()
    });
    time_query("IC6 tag co-occurrence", runs, || {
        ic6_tag_cooccurrence(&graph, seeds.person, &seed_tag_name).len()
    });
    time_query("IC8 recent replies", runs, || {
        ic8_recent_replies(&graph, seeds.person).len()
    });
    if let Some(msg) = seed_post {
        time_query("IS6 forum of message", runs, || {
            is6_forum_of_message(&graph, msg, reply_roots.as_deref().unwrap_or(&[])).is_some() as usize
        });
        time_query("IS7 replies of message", runs, || {
            is7_replies(&graph, msg).len()
        });
    }
    time_query("IC3 two countries", runs, || {
        ic3_friends_two_countries(&graph, seeds.person, "China", "Germany", days_from_civil(2010, 1, 1), 1500).len()
    });
    time_query("IC5 new groups", runs, || {
        ic5_new_groups(&graph, seeds.person, days_from_civil(2011, 1, 1)).len()
    });
    time_query("IC7 recent likers", runs, || {
        ic7_recent_likers(&graph, seeds.person).len()
    });
    time_query("IC10 friend recommend", runs, || {
        ic10_friend_recommend(&graph, seeds.person, 1).len()
    });
    time_query("IC11 job referral", runs, || {
        ic11_job_referral(&graph, seeds.person, &seed_country, 2030).len()
    });
    time_query("IC12 expert search", runs, || {
        ic12_expert_search(&graph, seeds.person, &seed_class_name).len()
    });
    if let Some(msg) = seed_post {
        time_query("IS4 message content", runs, || {
            is4_message_content(&graph, msg).is_some() as usize
        });
    }

    println!("\nAll IC1-IC14 and short reads IS1-IS7 implemented.");
    Ok(())
}
