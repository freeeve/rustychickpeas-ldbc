//! Load real LDBC SNB BI (SF1) data into rustychickpeas and time queries.
//!
//! Two query families run here:
//!   * **Faithful** BI queries (`Q1`, `Q2`, …) — translations of the official
//!     `ldbc/ldbc_snb_bi` Cypher queries, with their real filters, date
//!     parameters and aggregations. These read stored node properties through
//!     the public graph API, so the timings reflect rustychickpeas doing the
//!     actual analytical work (no query optimizer — each is hand-coded).
//!   * **Simplified** patterns (`BI1`–`BI6`) — the lighter namesakes the core
//!     repo's synthetic `ldbc_snb_bi` benchmark uses, kept for continuity with
//!     the synthetic-vs-real comparison.
//!
//! The dataset is the pipe-delimited, gzip-compressed CSV release from
//! <https://datasets.ldbcouncil.org/>. Node IDs there are i64 and only unique
//! within a node type (a Person and a Tag can share id 332), so we keep one
//! i64 -> u32 map per type and remap every relationship endpoint.

use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use csv::ReaderBuilder;
use rustychickpeas_core::{Column, Direction, GraphBuilder, GraphSnapshot, PropertyValue, ValueId};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// One Q1 output group: (year, isComment, lengthCategory, messageCount, sumLength).
type Q1Row = (i64, bool, u8, u64, i64);

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// algorithm). Used so date-range filters and N-day window arithmetic are plain
/// integer comparisons.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an LDBC creationDate ("2010-02-24T08:06:02.996+00:00") into
/// (year, days-since-epoch).
fn parse_date(s: &str) -> Option<(i64, i64)> {
    if s.len() < 10 {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..10].parse().ok()?;
    Some((y, days_from_civil(y, m, d)))
}

fn pi64(g: &GraphSnapshot, n: u32, k: &str) -> i64 {
    match g.prop(n, k) {
        Some(ValueId::I64(v)) => v,
        _ => 0,
    }
}

fn pbool(g: &GraphSnapshot, n: u32, k: &str) -> bool {
    matches!(g.prop(n, k), Some(ValueId::Bool(true)))
}

fn pstr<'a>(g: &'a GraphSnapshot, n: u32, k: &str) -> Option<&'a str> {
    match g.prop(n, k) {
        Some(ValueId::Str(s)) => g.resolve_string(s),
        _ => None,
    }
}

/// Minimal JSON string escaper (enough for LDBC tag/place names).
fn jstr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Write cross-check JSON (an array of row-arrays) to `<dir>/<name>`.
fn emit_json(dir: &str, name: &str, body: String) {
    let _ = std::fs::create_dir_all(dir);
    if let Err(e) = std::fs::write(format!("{dir}/{name}"), body) {
        eprintln!("emit_json {name}: {e}");
    }
}

/// Find a Tag node by its name property.
fn tag_by_name(g: &GraphSnapshot, name: &str) -> Option<u32> {
    g.nodes_with_label("Tag")
        .and_then(|tags| tags.iter().find(|&t| pstr(g, t, "name") == Some(name)))
}

/// Call `f` with the requested columns (in order) for every row across all
/// `part-*.csv.gz` files in `dir`. Returns the number of rows visited.
fn for_each_row(dir: &Path, cols: &[&str], mut f: impl FnMut(&[&str])) -> Result<u64> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {}", dir.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("gz"))
        .collect();
    files.sort();

    let mut count = 0u64;
    for path in files {
        let decoder = GzDecoder::new(BufReader::new(File::open(&path)?));
        let mut reader = ReaderBuilder::new()
            .delimiter(b'|')
            .has_headers(true)
            .from_reader(decoder);
        let headers = reader.headers()?.clone();
        let idx: Vec<usize> = cols
            .iter()
            .map(|c| {
                headers
                    .iter()
                    .position(|h| h == *c)
                    .ok_or_else(|| format!("column '{}' not in {:?}", c, headers))
            })
            .collect::<std::result::Result<_, _>>()?;

        let mut record = csv::StringRecord::new();
        while reader.read_record(&mut record)? {
            let vals: Vec<&str> = idx.iter().map(|&i| record.get(i).unwrap_or("")).collect();
            f(&vals);
            count += 1;
        }
    }
    Ok(count)
}

#[derive(Default)]
struct Stats {
    persons: u64,
    posts: u64,
    comments: u64,
    tags: u64,
    tag_classes: u64,
    edges: u64,
}

/// Store the message properties the faithful queries read: year, day-number,
/// length, and whether content is present (image-only posts have empty content).
fn set_message_props(b: &mut GraphBuilder, id: u32, creation: &str, content: &str, length: &str) {
    if let Some((year, day)) = parse_date(creation) {
        b.set_prop_i64(id, "year", year).unwrap();
        b.set_prop_i64(id, "day", day).unwrap();
    }
    b.set_prop_i64(id, "len", length.parse::<i64>().unwrap_or(0))
        .unwrap();
    b.set_prop_bool(id, "content", !content.is_empty()).unwrap();
}

/// Load the BI-relevant subgraph from an `initial_snapshot` directory.
fn load_graph(snapshot: &Path) -> Result<(GraphSnapshot, Stats)> {
    let dynamic = snapshot.join("dynamic");
    let static_ = snapshot.join("static");

    let mut builder = GraphBuilder::new(Some(3_000_000), Some(16_000_000));
    let mut next: u32 = 0;
    let mut person: HashMap<i64, u32> = HashMap::new();
    let mut tag: HashMap<i64, u32> = HashMap::new();
    let mut tagclass: HashMap<i64, u32> = HashMap::new();
    let mut post: HashMap<i64, u32> = HashMap::new();
    let mut comment: HashMap<i64, u32> = HashMap::new();
    let mut stats = Stats::default();

    // Static TagClass (load before Tag so HAS_TYPE edges resolve).
    for_each_row(&static_.join("TagClass"), &["id", "name"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["TagClass"]).unwrap();
            builder.set_prop_str(id, "name", v[1]).unwrap();
            tagclass.insert(lid, id);
        }
    })?;
    stats.tag_classes = tagclass.len() as u64;

    // Static Tag + Tag -[hasType]-> TagClass.
    for_each_row(
        &static_.join("Tag"),
        &["id", "name", "TypeTagClassId"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Tag"]).unwrap();
                builder.set_prop_str(id, "name", v[1]).unwrap();
                tag.insert(lid, id);
                if let Some(&class) = v[2].parse::<i64>().ok().and_then(|c| tagclass.get(&c)) {
                    builder.add_relationship(id, class, "hasType").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.tags = tag.len() as u64;

    // Persons (before Posts/Comments so hasCreator edges can resolve). Store
    // creationDate as epoch day (pday) and year*12+month (pym) for Q13.
    for_each_row(&dynamic.join("Person"), &["creationDate", "id"], |v| {
        if let Ok(lid) = v[1].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Person"]).unwrap();
            builder.set_prop_i64(id, "plid", lid).unwrap(); // LDBC id, for Q20 target
            if let Some((year, day)) = parse_date(v[0]) {
                let month = v[0]
                    .get(5..7)
                    .and_then(|m| m.parse::<i64>().ok())
                    .unwrap_or(1);
                builder.set_prop_i64(id, "pday", day).unwrap();
                builder.set_prop_i64(id, "pym", year * 12 + month).unwrap();
            }
            person.insert(lid, id);
        }
    })?;
    stats.persons = person.len() as u64;

    // Places (City/Country/Continent) + isPartOf hierarchy + Person isLocatedIn
    // City, for Q11's "persons in a country" filter.
    let mut place: HashMap<i64, u32> = HashMap::new();
    for_each_row(&static_.join("Place"), &["id", "name", "type"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &[v[2]]).unwrap(); // label = City/Country/Continent
            builder.set_prop_str(id, "name", v[1]).unwrap();
            builder.set_prop_i64(id, "lid", lid).unwrap(); // LDBC id, for Q19 city params
            place.insert(lid, id);
        }
    })?;
    for_each_row(&static_.join("Place"), &["id", "PartOfPlaceId"], |v| {
        let c = v[0].parse::<i64>().ok().and_then(|i| place.get(&i));
        let parent = v[1].parse::<i64>().ok().and_then(|i| place.get(&i));
        if let (Some(&c), Some(&p)) = (c, parent) {
            builder.add_relationship(c, p, "isPartOf").unwrap();
            stats.edges += 1;
        }
    })?;
    for_each_row(&dynamic.join("Person"), &["id", "LocationCityId"], |v| {
        let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
        let city = v[1].parse::<i64>().ok().and_then(|i| place.get(&i));
        if let (Some(&p), Some(&city)) = (p, city) {
            builder.add_relationship(p, city, "isLocatedIn").unwrap();
            stats.edges += 1;
        }
    })?;

    // Posts: node + properties (incl. language for Q12) + hasCreator.
    for_each_row(
        &dynamic.join("Post"),
        &[
            "id",
            "CreatorPersonId",
            "creationDate",
            "content",
            "length",
            "language",
        ],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Post"]).unwrap();
                set_message_props(&mut builder, id, v[2], v[3], v[4]);
                builder.set_prop_str(id, "lang", v[5]).unwrap();
                post.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_relationship(creator, id, "hasCreator").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.posts = post.len() as u64;

    // Comments: node + properties + hasCreator (Person -> Comment).
    for_each_row(
        &dynamic.join("Comment"),
        &["id", "CreatorPersonId", "creationDate", "content", "length"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Comment"]).unwrap();
                set_message_props(&mut builder, id, v[2], v[3], v[4]);
                comment.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_relationship(creator, id, "hasCreator").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.comments = comment.len() as u64;

    // Post -> Tag.
    for_each_row(
        &dynamic.join("Post_hasTag_Tag"),
        &["PostId", "TagId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| post.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&p), Some(&t)) = (p, t) {
                builder.add_relationship(p, t, "hasTag").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Comment -> Tag.
    for_each_row(
        &dynamic.join("Comment_hasTag_Tag"),
        &["CommentId", "TagId"],
        |v| {
            let c = v[0].parse::<i64>().ok().and_then(|i| comment.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&c), Some(&t)) = (c, t) {
                builder.add_relationship(c, t, "hasTag").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Person -> Tag interests.
    for_each_row(
        &dynamic.join("Person_hasInterest_Tag"),
        &["personId", "interestId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&p), Some(&t)) = (p, t) {
                builder.add_relationship(p, t, "hasInterest").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Comment -[replyOf]-> parent (Post or Comment). Separate pass so all
    // message ids are resolvable regardless of file order.
    for_each_row(
        &dynamic.join("Comment"),
        &["id", "ParentPostId", "ParentCommentId"],
        |v| {
            let c = v[0].parse::<i64>().ok().and_then(|i| comment.get(&i));
            let parent = if !v[1].is_empty() {
                v[1].parse::<i64>().ok().and_then(|i| post.get(&i))
            } else {
                v[2].parse::<i64>().ok().and_then(|i| comment.get(&i))
            };
            if let (Some(&c), Some(&p)) = (c, parent) {
                builder.add_relationship(c, p, "replyOf").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Person -[likes]-> Message (Post and Comment), for Q5/Q6.
    for_each_row(
        &dynamic.join("Person_likes_Post"),
        &["PersonId", "PostId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let m = v[1].parse::<i64>().ok().and_then(|i| post.get(&i));
            if let (Some(&p), Some(&m)) = (p, m) {
                builder.add_relationship(p, m, "likes").unwrap();
                stats.edges += 1;
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Person_likes_Comment"),
        &["PersonId", "CommentId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let m = v[1].parse::<i64>().ok().and_then(|i| comment.get(&i));
            if let (Some(&p), Some(&m)) = (p, m) {
                builder.add_relationship(p, m, "likes").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Person -[knows]- Person, undirected (both directions), with the edge's
    // creationDate stored as the "kd" property (epoch day) so Q11 can filter
    // knows edges by date during traversal. Uses the index returned by add_relationship
    // to set the property without an O(n) endpoint lookup.
    for_each_row(
        &dynamic.join("Person_knows_Person"),
        &["creationDate", "Person1Id", "Person2Id"],
        |v| {
            let day = parse_date(v[0]).map(|(_, d)| d).unwrap_or(0);
            let a = v[1].parse::<i64>().ok().and_then(|i| person.get(&i));
            let b = v[2].parse::<i64>().ok().and_then(|i| person.get(&i));
            if let (Some(&a), Some(&b)) = (a, b) {
                let i1 = builder.add_relationship(a, b, "knows").unwrap();
                builder.set_relationship_props_by_index(i1, &[("kd", PropertyValue::Integer(day))]);
                let i2 = builder.add_relationship(b, a, "knows").unwrap();
                builder.set_relationship_props_by_index(i2, &[("kd", PropertyValue::Integer(day))]);
                stats.edges += 2;
            }
        },
    )?;

    // Organisations (Company/University) + Person workAt Company + Person studyAt
    // University (classYear stored as an edge property), for Q20.
    let mut org: HashMap<i64, u32> = HashMap::new();
    for_each_row(
        &static_.join("Organisation"),
        &["id", "type", "name"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &[v[1]]).unwrap(); // label = Company / University
                builder.set_prop_str(id, "name", v[2]).unwrap();
                org.insert(lid, id);
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Person_workAt_Company"),
        &["PersonId", "CompanyId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let c = v[1].parse::<i64>().ok().and_then(|i| org.get(&i));
            if let (Some(&p), Some(&c)) = (p, c) {
                builder.add_relationship(p, c, "workAt").unwrap();
                stats.edges += 1;
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Person_studyAt_University"),
        &["PersonId", "UniversityId", "classYear"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let u = v[1].parse::<i64>().ok().and_then(|i| org.get(&i));
            if let (Some(&p), Some(&u)) = (p, u) {
                let cy = v[2].parse::<i64>().unwrap_or(0);
                let idx = builder.add_relationship(p, u, "studyAt").unwrap();
                builder.set_relationship_props_by_index(idx, &[("cy", PropertyValue::Integer(cy))]);
                stats.edges += 1;
            }
        },
    )?;

    Ok((builder.finalize(None), stats))
}

// ============ Faithful LDBC SNB BI queries ============

/// Q1 — Posting summary. Messages before `cutoff_day` (with content), grouped by
/// (year, isComment, length-category); reports counts, average/sum length and
/// share of all messages before the cutoff.
/// Returns (group rows, total-message-count). Cypher: bi-1.cypher.
fn q1_posting_summary(g: &GraphSnapshot, cutoff_day: i64) -> (Vec<Q1Row>, u64) {
    // Resolve each property's column once, hoisting the per-message key-string
    // interning + columns lookup out of the multi-million-row scan (the
    // dominant cost; behavior is identical to per-row `prop()` reads).
    let col = |k: &str| g.property_key_from_str(k).and_then(|id| g.columns.get(&id));
    let (day_col, content_col, len_col, year_col) =
        (col("day"), col("content"), col("len"), col("year"));
    let get_i64 = |c: Option<&Column>, n: u32| match c.and_then(|c| c.get(n)) {
        Some(ValueId::I64(v)) => v,
        _ => 0,
    };

    let mut total = 0u64;
    let mut groups: HashMap<(i64, bool, u8), (u64, i64)> = HashMap::new();
    for (label, is_comment) in [("Post", false), ("Comment", true)] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                if get_i64(day_col, msg) >= cutoff_day {
                    continue;
                }
                total += 1;
                if !matches!(content_col.and_then(|c| c.get(msg)), Some(ValueId::Bool(true))) {
                    continue;
                }
                let len = get_i64(len_col, msg);
                let cat = if len < 40 {
                    0
                } else if len < 80 {
                    1
                } else if len < 160 {
                    2
                } else {
                    3
                };
                let e = groups
                    .entry((get_i64(year_col, msg), is_comment, cat))
                    .or_insert((0, 0));
                e.0 += 1;
                e.1 += len;
            }
        }
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
fn q2_tag_evolution(
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

    // Tags of that class.
    let mut qualifying: HashSet<u32> = HashSet::new();
    if let Some(tags) = g.nodes_with_label("Tag") {
        for t in tags.iter() {
            if g.neighbors_by_type(t, Direction::Outgoing, &["hasType"]).contains(&target) {
                qualifying.insert(t);
            }
        }
    }

    let (w1_lo, w1_hi) = (date0_day, date0_day + 100);
    let (w2_lo, w2_hi) = (date0_day + 100, date0_day + 200);
    let mut c1: HashMap<u32, u64> = HashMap::new();
    let mut c2: HashMap<u32, u64> = HashMap::new();
    // Resolve the day column once; the window filter scans every message.
    let day_col = g.property_key_from_str("day").and_then(|id| g.columns.get(&id));
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let day = match day_col.and_then(|c| c.get(msg)) {
                    Some(ValueId::I64(v)) => v,
                    _ => 0,
                };
                let in1 = w1_lo <= day && day < w1_hi;
                let in2 = w2_lo <= day && day < w2_hi;
                if !in1 && !in2 {
                    continue;
                }
                for t in g.neighbors_by_type(msg, Direction::Outgoing, &["hasTag"]) {
                    if qualifying.contains(&t) {
                        *(if in1 { &mut c1 } else { &mut c2 }).entry(t).or_insert(0) += 1;
                    }
                }
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
fn q7_related_topics(g: &GraphSnapshot, tag_name: &str) -> Vec<(String, usize)> {
    let target = g
        .nodes_with_label("Tag")
        .and_then(|tags| tags.iter().find(|&t| pstr(g, t, "name") == Some(tag_name)));
    let Some(target) = target else {
        return Vec::new();
    };

    let mut related: HashMap<u32, HashSet<u32>> = HashMap::new();
    for msg in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        for comment in g.neighbors_by_type(msg, Direction::Incoming, &["replyOf"]) {
            let ctags = g.neighbors_by_type(comment, Direction::Outgoing, &["hasTag"]);
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
fn q12_message_counts(
    g: &GraphSnapshot,
    min_day: i64,
    len_thr: i64,
    langs: &[&str],
) -> Vec<(u64, u64)> {
    let posts = g.nodes_with_label("Post");
    // Root-post language: the message itself if it is a Post, else walk replyOf
    // up to the root Post (depth-capped against pathological chains).
    let root_lang = |start: u32| -> Option<&str> {
        let mut n = start;
        for _ in 0..64 {
            if posts.is_some_and(|p| p.contains(n)) {
                return pstr(g, n, "lang");
            }
            n = *g.neighbors_by_type(n, Direction::Outgoing, &["replyOf"]).first()?;
        }
        None
    };

    let mut per_person: HashMap<u32, u64> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                if pi64(g, msg, "day") <= min_day
                    || !pbool(g, msg, "content")
                    || pi64(g, msg, "len") >= len_thr
                {
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
fn q5_active_posters(g: &GraphSnapshot, tag_name: &str) -> Vec<(u32, u64, u64, u64, u64)> {
    let Some(target) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let mut agg: HashMap<u32, (u64, u64, u64)> = HashMap::new(); // person -> (msgs, replies, likes)
    for message in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        let likes = g.neighbors_by_type(message, Direction::Incoming, &["likes"]).len() as u64;
        let replies = g.neighbors_by_type(message, Direction::Incoming, &["replyOf"]).len() as u64;
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
    rows.sort_by(|a, b| b.4.cmp(&a.4).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q6 — Most authoritative users on a topic. For each creator of tagged messages
/// (person1), find who liked those messages (person2), and sum the likes those
/// person2s received on their own messages. Cypher: bi-6.cypher.
fn q6_authoritative(g: &GraphSnapshot, tag_name: &str) -> Vec<(u32, u64)> {
    let Some(target) = tag_by_name(g, tag_name) else {
        return Vec::new();
    };
    let mut p1_to_p2: HashMap<u32, HashSet<u32>> = HashMap::new();
    for message1 in g.neighbors_by_type(target, Direction::Incoming, &["hasTag"]) {
        let likers = g.neighbors_by_type(message1, Direction::Incoming, &["likes"]);
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
                .map(|m2| g.neighbors_by_type(m2, Direction::Incoming, &["likes"]).len() as u64)
                .sum();
            (p1, score)
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q8 — Central Person for a Tag. Score persons by interest in the tag (×100) +
/// messages they made with the tag in a date window, then add their friends'
/// scores. Returns (person, score, friendsScore), top 100 by score+friendsScore.
/// Cypher: bi-8.cypher.
fn q8_central_person(
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
                .iter()
                .map(|f| score.get(f).copied().unwrap_or(0))
                .sum();
            (p, s, fs)
        })
        .collect();
    rows.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q11 — Friend triangles. Count triangles in the `knows` graph among persons of
/// a given country where every edge was created within a date window. This is
/// the query that motivated the core `out_edges` API: it reads each knows edge's
/// `creationDate` (`kd`) during traversal via the edge's CSR position.
/// Cypher: bi-11.cypher.
fn q11_friend_triangles(
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
    let mut adj: HashMap<u32, HashSet<u32>> = HashMap::new();
    for &a in &in_country {
        for e in g.relationships(a, Direction::Outgoing, &["knows"]) {
            if !in_country.contains(&e.neighbor) {
                continue;
            }
            let kd = match g.relationship_property(e.pos, "kd") {
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
fn q9_thread_initiators(g: &GraphSnapshot, start_day: i64, end_day: i64) -> Vec<(u32, u64, u64)> {
    let mut per_person: HashMap<u32, (u64, u64)> = HashMap::new(); // (threads, messages)
    if let Some(posts) = g.nodes_with_label("Post") {
        for post in posts.iter() {
            let pd = pi64(g, post, "day");
            if pd < start_day || pd > end_day {
                continue;
            }
            let Some(&creator) = g.neighbors_by_type(post, Direction::Incoming, &["hasCreator"]).first() else {
                continue;
            };
            // Walk the post's reply tree; replies are created after their parent,
            // so prune any node past end_day (its whole subtree is later).
            let mut msgs = 0u64;
            let mut stack = vec![post];
            while let Some(n) = stack.pop() {
                let d = pi64(g, n, "day");
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
    rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

/// Q13 — Zombies in a country. Zombies are low-activity persons (created before
/// endDate with under one message per month). Score each by the share of likes
/// on their messages that come from other zombies. Cypher: bi-13.cypher.
fn q13_zombies(
    g: &GraphSnapshot,
    country_name: &str,
    end_day: i64,
    end_ym: i64,
) -> Vec<(u32, u64, u64)> {
    let country = g.nodes_with_label("Country").and_then(|cs| {
        cs.iter()
            .find(|&c| pstr(g, c, "name") == Some(country_name))
    });
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
                .iter()
                .filter(|&&m| pi64(g, m, "day") < end_day)
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
            .then(a.0.cmp(&b.0))
    });
    rows.truncate(100);
    rows
}

/// End-to-end validation of the core `dijkstra` primitive on the real `knows`
/// graph: shortest-hop reachability from one person (unit weights), returning
/// (persons reachable, eccentricity in hops). Not a faithful BI query — Q19/Q20
/// would need a derived interaction-weight graph — but it exercises dijkstra +
/// path reconstruction at SF scale.
fn knows_reachability(g: &GraphSnapshot) -> (usize, u32) {
    let persons: Vec<u32> = g
        .nodes_with_label("Person")
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    let Some(&source) = persons.first() else {
        return (0, 0);
    };
    let sp = g.dijkstra(source, Direction::Both, &["knows"], None, |_, _| 1.0);
    let reachable = persons.iter().filter(|&&p| sp.reached(p)).count();
    let ecc = persons
        .iter()
        .filter_map(|&p| sp.distance(p))
        .fold(0.0_f64, f64::max) as u32;
    (reachable, ecc)
}

/// Find a place node (City/Country/...) by its LDBC id.
fn place_by_lid(g: &GraphSnapshot, lid: i64) -> Option<u32> {
    ["City", "Country"].iter().find_map(|label| {
        g.nodes_with_label(label)
            .and_then(|ns| ns.iter().find(|&n| pi64(g, n, "lid") == lid))
    })
}

/// Precompute the per-pair person interaction counts for Q19: the number of
/// reply interactions between the message creators of each (undirected) pair.
/// This is the weighted "projected graph" Q19 runs over; building it once
/// mirrors Q19's precomputation variant.
fn build_interaction_map(g: &GraphSnapshot) -> HashMap<(u32, u32), u32> {
    let mut interaction: HashMap<(u32, u32), u32> = HashMap::new();
    if let Some(comments) = g.nodes_with_label("Comment") {
        for c in comments.iter() {
            let Some(&a) = g.neighbors_by_type(c, Direction::Incoming, &["hasCreator"]).first() else {
                continue;
            };
            for parent in g.neighbors_by_type(c, Direction::Outgoing, &["replyOf"]) {
                if let Some(&b) = g.neighbors_by_type(parent, Direction::Incoming, &["hasCreator"]).first() {
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
fn q19_interaction_path(
    g: &GraphSnapshot,
    city1: u32,
    city2: u32,
    interaction: &HashMap<(u32, u32), u32>,
) -> Vec<(u32, u32, f64)> {
    let c1 = g.neighbors_by_type(city1, Direction::Incoming, &["isLocatedIn"]);
    let c2: HashSet<u32> = g
        .neighbors_by_type(city2, Direction::Incoming, &["isLocatedIn"])
        .into_iter()
        .collect();
    let mut results: Vec<(u32, u32, f64)> = Vec::new();
    for p1 in c1 {
        let sp = g.dijkstra(p1, Direction::Both, &["knows"], None, |from, rel| {
            match interaction.get(&(from.min(rel.neighbor), from.max(rel.neighbor))) {
                Some(&n) if n > 0 => 1.0 / n as f64,
                _ => f64::INFINITY, // know each other but never interacted: no edge
            }
        });
        for &p2 in &c2 {
            if let Some(d) = sp.distance(p2) {
                if d.is_finite() {
                    results.push((p1, p2, d));
                }
            }
        }
    }
    results.sort_by(|a, b| {
        a.2.partial_cmp(&b.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    results.truncate(20);
    results
}

/// Find an Organisation node (Company/University) by name.
fn org_by_name(g: &GraphSnapshot, label: &str, name: &str) -> Option<u32> {
    g.nodes_with_label(label)
        .and_then(|ns| ns.iter().find(|&n| pstr(g, n, "name") == Some(name)))
}

/// Find a Person node by its LDBC id.
fn person_by_plid(g: &GraphSnapshot, plid: i64) -> Option<u32> {
    g.nodes_with_label("Person")
        .and_then(|ns| ns.iter().find(|&n| pi64(g, n, "plid") == plid))
}

/// Per-person study records (university, classYear), read from studyAt edges and
/// their classYear edge property.
fn build_studyat(g: &GraphSnapshot) -> HashMap<u32, Vec<(u32, i64)>> {
    let mut m: HashMap<u32, Vec<(u32, i64)>> = HashMap::new();
    if let Some(persons) = g.nodes_with_label("Person") {
        for p in persons.iter() {
            let recs: Vec<(u32, i64)> = g
                .relationships(p, Direction::Outgoing, &["studyAt"])
                .iter()
                .map(|r| {
                    let cy = match g.relationship_property(r.pos, "cy") {
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
fn build_study_weight_map(
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
fn q20_recruitment(
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
    results.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    results.truncate(20);
    results
}

// ============ Simplified analytical patterns (synthetic-benchmark parity) ============

fn bi1_tag_evolution(g: &GraphSnapshot) -> usize {
    let mut pairs: HashMap<(u32, u32), u32> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let tags = g.neighbors_by_type(msg, Direction::Outgoing, &["hasTag"]);
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

fn bi3_popular_topics(g: &GraphSnapshot) -> usize {
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

fn top_creators(g: &GraphSnapshot, label: &str) -> usize {
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

/// Median wall-clock over `runs` timed iterations (after one warmup).
fn time_query(name: &str, runs: usize, mut q: impl FnMut() -> usize) {
    let warm = q();
    let mut samples: Vec<u128> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        let _ = q();
        samples.push(t.elapsed().as_micros());
    }
    samples.sort_unstable();
    let median_ms = samples[samples.len() / 2] as f64 / 1000.0;
    println!("{name:<34} {median_ms:>9.2} ms   (result={warm})");
}

fn main() -> Result<()> {
    let default = PathBuf::from(
        "data/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot",
    );
    let snapshot = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(default);
    if !snapshot.join("dynamic").is_dir() {
        return Err(format!(
            "no 'dynamic' dir under {}; pass the initial_snapshot path as arg 1",
            snapshot.display()
        )
        .into());
    }

    eprintln!("Loading LDBC graph from {} ...", snapshot.display());
    let t = Instant::now();
    let (graph, s) = load_graph(&snapshot)?;
    let load_secs = t.elapsed().as_secs_f64();

    let nodes = s.persons + s.posts + s.comments + s.tags + s.tag_classes;
    println!("\n=== LDBC SNB BI — SF1 (real data) ===");
    println!(
        "Loaded {nodes} nodes ({} persons, {} posts, {} comments, {} tags, {} tagclasses)",
        s.persons, s.posts, s.comments, s.tags, s.tag_classes
    );
    println!("       {} edges in {load_secs:.1}s\n", s.edges);

    // --- Faithful LDBC BI queries (official Cypher params) ---
    // Set LDBC_EMIT_JSON=<dir> to dump Q1/Q2 result rows for the Kùzu
    // cross-check (and skip the slower downstream queries).
    let emit = std::env::var("LDBC_EMIT_JSON").ok();

    println!("Faithful LDBC BI queries:");
    let q1_cutoff = days_from_civil(2011, 12, 1);
    let t_q1 = Instant::now();
    let (q1_rows, q1_total) = q1_posting_summary(&graph, q1_cutoff);
    let q1_ms = t_q1.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  Q1 posting summary: {} groups over {} messages before 2011-12-01  [{q1_ms:.1} ms]",
        q1_rows.len(),
        q1_total
    );
    for (y, is_c, cat, n, sum) in q1_rows.iter().take(4) {
        let avg = *sum as f64 / *n as f64;
        let kind = if *is_c { "Comment" } else { "Post" };
        println!("     {y} {kind:<7} lenCat={cat}  count={n}  avgLen={avg:.1}");
    }
    let q2_date = days_from_civil(2012, 6, 1);
    let t_q2 = Instant::now();
    let q2_rows = q2_tag_evolution(&graph, q2_date, "MusicalArtist");
    let q2_ms = t_q2.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  Q2 tag evolution (MusicalArtist, 2012-06-01): {} tags  [{q2_ms:.1} ms]",
        q2_rows.len()
    );
    for (name, n1, n2, diff) in q2_rows.iter().take(3) {
        println!("     {name:<30} w1={n1} w2={n2} diff={diff}");
    }

    if let Some(dir) = emit.as_deref() {
        let mut s1 = String::from("[");
        for (i, (y, c, cat, n, sum)) in q1_rows.iter().enumerate() {
            if i > 0 {
                s1.push(',');
            }
            s1.push_str(&format!(
                "[{y},{},{cat},{n},{sum}]",
                if *c { "true" } else { "false" }
            ));
        }
        s1.push(']');
        emit_json(dir, "q1.rust.json", s1);

        let mut s2 = String::from("[");
        for (i, (name, n1, n2, diff)) in q2_rows.iter().enumerate() {
            if i > 0 {
                s2.push(',');
            }
            s2.push_str(&format!("[{},{n1},{n2},{diff}]", jstr(name)));
        }
        s2.push(']');
        emit_json(dir, "q2.rust.json", s2);

        eprintln!("emitted Q1/Q2 cross-check JSON to {dir}; skipping downstream queries");
        return Ok(());
    }

    let q7_rows = q7_related_topics(&graph, "Enrique_Iglesias");
    println!(
        "  Q7 related topics (Enrique_Iglesias): {} related tags",
        q7_rows.len()
    );
    for (name, c) in q7_rows.iter().take(3) {
        println!("     {name:<30} comments={c}");
    }
    let q12_min = days_from_civil(2010, 7, 22);
    let q12_rows = q12_message_counts(&graph, q12_min, 20, &["ar", "hu"]);
    println!(
        "  Q12 message counts (len<20, after 2010-07-22, lang ar/hu): {} buckets",
        q12_rows.len()
    );
    for (mc, pc) in q12_rows.iter().take(3) {
        println!("     messageCount={mc} -> persons={pc}");
    }
    let q5_rows = q5_active_posters(&graph, "Abbas_I_of_Persia");
    println!(
        "  Q5 active posters (Abbas_I_of_Persia): {} persons",
        q5_rows.len()
    );
    for (_p, m, r, l, score) in q5_rows.iter().take(3) {
        println!("     msgs={m} replies={r} likes={l} score={score}");
    }
    let q6_rows = q6_authoritative(&graph, "Arnold_Schwarzenegger");
    println!(
        "  Q6 authoritative users (Arnold_Schwarzenegger): {} persons",
        q6_rows.len()
    );
    for (_p, score) in q6_rows.iter().take(3) {
        println!("     authorityScore={score}");
    }
    let q8_start = days_from_civil(2011, 7, 20);
    let q8_end = days_from_civil(2011, 7, 25);
    let q8_rows = q8_central_person(&graph, "Che_Guevara", q8_start, q8_end);
    println!(
        "  Q8 central person (Che_Guevara, 2011-07-20..25): {} persons",
        q8_rows.len()
    );
    for (_p, s, fs) in q8_rows.iter().take(3) {
        println!("     score={s} friendsScore={fs}");
    }
    let q11_start = days_from_civil(2012, 9, 29);
    let q11_end = days_from_civil(2013, 1, 1);
    let q11_count = q11_friend_triangles(&graph, "India", q11_start, q11_end);
    println!("  Q11 friend triangles (India, 2012-09-29..2013-01-01): {q11_count} triangles");
    let q9_start = days_from_civil(2011, 10, 1);
    let q9_end = days_from_civil(2011, 10, 15);
    let q9_rows = q9_thread_initiators(&graph, q9_start, q9_end);
    println!(
        "  Q9 thread initiators (2011-10-01..15): {} persons",
        q9_rows.len()
    );
    let q13_end = days_from_civil(2013, 1, 1);
    let q13_ym = 2013 * 12 + 1;
    let q13_rows = q13_zombies(&graph, "France", q13_end, q13_ym);
    println!(
        "  Q13 zombies (France, before 2013-01-01): {} zombies",
        q13_rows.len()
    );
    let (reach, ecc) = knows_reachability(&graph);
    println!(
        "  dijkstra knows-reachability from person[0]: {reach} reachable, eccentricity {ecc} hops"
    );
    let interaction = build_interaction_map(&graph);
    let q19_cities = place_by_lid(&graph, 669).zip(place_by_lid(&graph, 648));
    match q19_cities {
        Some((c1, c2)) => {
            let q19 = q19_interaction_path(&graph, c1, c2, &interaction);
            println!(
                "  Q19 interaction path (cities 669<->648): {} pairs over {} interaction edges",
                q19.len(),
                interaction.len()
            );
            if let Some((p1, p2, w)) = q19.first() {
                println!("     best: person {p1} -> person {p2}, total weight {w:.4}");
            }
        }
        None => println!("  Q19: city 669 or 648 not present in dataset"),
    }
    let studyat = build_studyat(&graph);
    let study_wm = build_study_weight_map(&graph, &studyat);
    let q20_args = org_by_name(&graph, "Company", "Falcon_Air").zip(person_by_plid(&graph, 66));
    match q20_args {
        Some((co, p2)) => {
            let q20 = q20_recruitment(&graph, co, p2, &study_wm);
            println!(
                "  Q20 recruitment (Falcon_Air -> person 66): {} candidates over {} study edges",
                q20.len(),
                study_wm.len()
            );
            if let Some((p1, w)) = q20.first() {
                println!("     best: person {p1}, total weight {w:.1}");
            }
        }
        None => println!("  Q20: company Falcon_Air or person 66 not present"),
    }
    println!();

    let runs = 5;
    println!("Timings (median of {runs}):");
    time_query("Q1 posting summary", runs, || {
        q1_posting_summary(&graph, q1_cutoff).0.len()
    });
    time_query("Q2 tag evolution", runs, || {
        q2_tag_evolution(&graph, q2_date, "MusicalArtist").len()
    });
    time_query("Q7 related topics", runs, || {
        q7_related_topics(&graph, "Enrique_Iglesias").len()
    });
    time_query("Q12 message counts", runs, || {
        q12_message_counts(&graph, q12_min, 20, &["ar", "hu"]).len()
    });
    time_query("Q5 active posters", runs, || {
        q5_active_posters(&graph, "Abbas_I_of_Persia").len()
    });
    time_query("Q6 authoritative users", runs, || {
        q6_authoritative(&graph, "Arnold_Schwarzenegger").len()
    });
    time_query("Q8 central person", runs, || {
        q8_central_person(&graph, "Che_Guevara", q8_start, q8_end).len()
    });
    time_query("Q11 friend triangles", runs, || {
        q11_friend_triangles(&graph, "India", q11_start, q11_end) as usize
    });
    time_query("Q9 thread initiators", runs, || {
        q9_thread_initiators(&graph, q9_start, q9_end).len()
    });
    time_query("Q13 zombies", runs, || {
        q13_zombies(&graph, "France", q13_end, q13_ym).len()
    });
    time_query("dijkstra knows reachability", runs, || {
        knows_reachability(&graph).0
    });
    if let Some((c1, c2)) = q19_cities {
        time_query("Q19 interaction path", runs, || {
            q19_interaction_path(&graph, c1, c2, &interaction).len()
        });
    }
    if let Some((co, p2)) = q20_args {
        time_query("Q20 recruitment", runs, || {
            q20_recruitment(&graph, co, p2, &study_wm).len()
        });
    }
    // Simplified patterns (parity with the synthetic benchmark).
    time_query("BI1 tag co-evolution (simpl.)", runs, || {
        bi1_tag_evolution(&graph)
    });
    time_query("BI3 popular topics (simpl.)", runs, || {
        bi3_popular_topics(&graph)
    });
    time_query("BI4 top commenters (simpl.)", runs, || {
        top_creators(&graph, "Comment")
    });
    time_query("BI5 active users (simpl.)", runs, || {
        top_creators(&graph, "Post")
    });

    Ok(())
}
