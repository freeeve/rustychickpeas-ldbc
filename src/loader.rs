//! CSV ingest — build the BI-relevant subgraph from an `initial_snapshot` dir.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use csv::ReaderBuilder;
use flate2::read::GzDecoder;
use rustychickpeas_core::{GraphBuilder, GraphSnapshot, PropertyValue};

use crate::harness::Result;
use crate::props::{parse_date, parse_ms};

/// Call `f` with the requested columns (in order) for every row across all
/// `part-*.csv.gz` files in `dir`. Returns the number of rows visited.
pub fn for_each_row(dir: &Path, cols: &[&str], mut f: impl FnMut(&[&str])) -> Result<u64> {
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
pub struct Stats {
    pub persons: u64,
    pub posts: u64,
    pub comments: u64,
    pub tags: u64,
    pub tag_classes: u64,
    pub rels: u64,
}

/// Store the message properties the faithful queries read: year, day-number,
/// length, and whether content is present (image-only posts have empty content).
pub fn set_message_props(
    b: &mut GraphBuilder,
    id: u32,
    creation: &str,
    content: &str,
    length: &str,
) {
    if let Some((year, day)) = parse_date(creation) {
        b.set_prop_i64(id, "year", year).unwrap();
        b.set_prop_i64(id, "day", day).unwrap();
    }
    b.set_prop_i64(id, "ms", parse_ms(creation)).unwrap();
    b.set_prop_i64(id, "len", length.parse::<i64>().unwrap_or(0))
        .unwrap();
    // content presence as 0/1 i64 (q12's only reader) so the native aggregate
    // kernel — which filters dense i64 columns — can apply it directly.
    b.set_prop_i64(id, "content", i64::from(!content.is_empty()))
        .unwrap();
}

/// Load the BI-relevant subgraph from an `initial_snapshot` directory.
pub fn load_graph(snapshot: &Path) -> Result<(GraphSnapshot, Stats)> {
    load_graph_opts(snapshot, false)
}

/// Like [`load_graph`], but with `load_content` also stores each message's
/// content text (Post falls back to `imageFile` when content is empty) as the
/// `ctext` property for the Interactive IS4 read. Off by default so BI/SPB
/// loads stay lean — content text is ~hundreds of MB at SF1 and the BI queries
/// only need the `hasContent` bool, which is always stored.
pub fn load_graph_opts(snapshot: &Path, load_content: bool) -> Result<(GraphSnapshot, Stats)> {
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

    // Static TagClass (load before Tag so HAS_TYPE rels resolve).
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

    // TagClass -[isSubclassOf]-> parent TagClass (the class hierarchy), for IC12.
    for_each_row(
        &static_.join("TagClass"),
        &["id", "SubclassOfTagClassId"],
        |v| {
            let c = v[0].parse::<i64>().ok().and_then(|i| tagclass.get(&i));
            let parent = v[1].parse::<i64>().ok().and_then(|i| tagclass.get(&i));
            if let (Some(&c), Some(&p)) = (c, parent) {
                builder.add_relationship(c, p, "isSubclassOf").unwrap();
                stats.rels += 1;
            }
        },
    )?;

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
                    stats.rels += 1;
                }
            }
        },
    )?;
    stats.tags = tag.len() as u64;

    // Persons (before Posts/Comments so hasCreator rels can resolve). Store
    // creationDate as epoch day (pday) and year*12+month (pym) for Q13, plus
    // first/last name (fname/lname) for the Interactive workload's IC1/IS1.
    for_each_row(
        &dynamic.join("Person"),
        &[
            "creationDate",
            "id",
            "firstName",
            "lastName",
            "gender",
            "birthday",
        ],
        |v| {
            if let Ok(lid) = v[1].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Person"]).unwrap();
                builder.set_prop_i64(id, "plid", lid).unwrap(); // LDBC id, for Q20 target
                builder.set_prop_str(id, "fname", v[2]).unwrap();
                builder.set_prop_str(id, "lname", v[3]).unwrap();
                builder.set_prop_str(id, "gender", v[4]).unwrap(); // IC10 output
                if v[5].len() >= 10 {
                    // birthday MM/DD for IC10's day-window filter.
                    builder
                        .set_prop_i64(id, "bmon", v[5][5..7].parse().unwrap_or(0))
                        .unwrap();
                    builder
                        .set_prop_i64(id, "bdom", v[5][8..10].parse().unwrap_or(0))
                        .unwrap();
                }
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
        },
    )?;
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
            stats.rels += 1;
        }
    })?;
    for_each_row(&dynamic.join("Person"), &["id", "LocationCityId"], |v| {
        let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
        let city = v[1].parse::<i64>().ok().and_then(|i| place.get(&i));
        if let (Some(&p), Some(&city)) = (p, city) {
            builder.add_relationship(p, city, "isLocatedIn").unwrap();
            stats.rels += 1;
        }
    })?;

    // Forums (title + creationDate day) + hasModerator + hasMember, for the
    // Forum-based queries (Q3/Q4/Q15/Q17). containerOf is added after Posts.
    let mut forum: HashMap<i64, u32> = HashMap::new();
    for_each_row(
        &dynamic.join("Forum"),
        &["id", "title", "creationDate", "ModeratorPersonId"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Forum"]).unwrap();
                builder.set_prop_i64(id, "flid", lid).unwrap(); // LDBC id, for Q3 output
                builder.set_prop_str(id, "title", v[1]).unwrap();
                if let Some((_, day)) = parse_date(v[2]) {
                    builder.set_prop_i64(id, "fday", day).unwrap();
                }
                forum.insert(lid, id);
                if let Some(&m) = v[3].parse::<i64>().ok().and_then(|p| person.get(&p)) {
                    builder.add_relationship(id, m, "hasModerator").unwrap();
                    stats.rels += 1;
                }
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Forum_hasMember_Person"),
        &["creationDate", "ForumId", "PersonId"],
        |v| {
            let f = v[1].parse::<i64>().ok().and_then(|i| forum.get(&i));
            let p = v[2].parse::<i64>().ok().and_then(|i| person.get(&i));
            if let (Some(&f), Some(&p)) = (f, p) {
                let day = parse_date(v[0]).map(|(_, d)| d).unwrap_or(0);
                let idx = builder.add_relationship(f, p, "hasMember").unwrap();
                builder
                    .set_relationship_props_by_index(idx, &[("hd", PropertyValue::Integer(day))]); // IC5 join date
                stats.rels += 1;
            }
        },
    )?;

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
            "imageFile",
        ],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Post"]).unwrap();
                set_message_props(&mut builder, id, v[2], v[3], v[4]);
                builder.set_prop_str(id, "lang", v[5]).unwrap();
                if load_content {
                    // IS4 content; image-only posts have empty content -> imageFile.
                    let text = if v[3].is_empty() { v[6] } else { v[3] };
                    builder.set_prop_str(id, "ctext", text).unwrap();
                }
                post.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_relationship(creator, id, "hasCreator").unwrap();
                    stats.rels += 1;
                }
            }
        },
    )?;
    stats.posts = post.len() as u64;

    // Forum -[containerOf]-> Post (from Post.ContainerForumId).
    for_each_row(&dynamic.join("Post"), &["id", "ContainerForumId"], |v| {
        let p = v[0].parse::<i64>().ok().and_then(|i| post.get(&i));
        let f = v[1].parse::<i64>().ok().and_then(|i| forum.get(&i));
        if let (Some(&p), Some(&f)) = (p, f) {
            builder.add_relationship(f, p, "containerOf").unwrap();
            stats.rels += 1;
        }
    })?;

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
                if load_content {
                    builder.set_prop_str(id, "ctext", v[3]).unwrap(); // IS4 content
                }
                comment.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_relationship(creator, id, "hasCreator").unwrap();
                    stats.rels += 1;
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
                stats.rels += 1;
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
                stats.rels += 1;
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
                stats.rels += 1;
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
                stats.rels += 1;
            }
        },
    )?;

    // Post/Comment -[msgCountry]-> Country (LocationCountryId), for IC3.
    for_each_row(&dynamic.join("Post"), &["id", "LocationCountryId"], |v| {
        let m = v[0].parse::<i64>().ok().and_then(|i| post.get(&i));
        let c = v[1].parse::<i64>().ok().and_then(|i| place.get(&i));
        if let (Some(&m), Some(&c)) = (m, c) {
            builder.add_relationship(m, c, "msgCountry").unwrap();
            stats.rels += 1;
        }
    })?;
    for_each_row(
        &dynamic.join("Comment"),
        &["id", "LocationCountryId"],
        |v| {
            let m = v[0].parse::<i64>().ok().and_then(|i| comment.get(&i));
            let c = v[1].parse::<i64>().ok().and_then(|i| place.get(&i));
            if let (Some(&m), Some(&c)) = (m, c) {
                builder.add_relationship(m, c, "msgCountry").unwrap();
                stats.rels += 1;
            }
        },
    )?;

    // Person -[likes]-> Message (Post and Comment), for Q5/Q6.
    for_each_row(
        &dynamic.join("Person_likes_Post"),
        &["creationDate", "PersonId", "PostId"],
        |v| {
            let p = v[1].parse::<i64>().ok().and_then(|i| person.get(&i));
            let m = v[2].parse::<i64>().ok().and_then(|i| post.get(&i));
            if let (Some(&p), Some(&m)) = (p, m) {
                let idx = builder.add_relationship(p, m, "likes").unwrap();
                builder.set_relationship_props_by_index(
                    idx,
                    &[("ld", PropertyValue::Integer(parse_ms(v[0])))],
                ); // IC7
                stats.rels += 1;
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Person_likes_Comment"),
        &["creationDate", "PersonId", "CommentId"],
        |v| {
            let p = v[1].parse::<i64>().ok().and_then(|i| person.get(&i));
            let m = v[2].parse::<i64>().ok().and_then(|i| comment.get(&i));
            if let (Some(&p), Some(&m)) = (p, m) {
                let idx = builder.add_relationship(p, m, "likes").unwrap();
                builder.set_relationship_props_by_index(
                    idx,
                    &[("ld", PropertyValue::Integer(parse_ms(v[0])))],
                ); // IC7
                stats.rels += 1;
            }
        },
    )?;

    // Person -[knows]- Person, undirected (both directions), with the rel's
    // creationDate stored as the "kd" property (epoch day) so Q11 can filter
    // knows rels by date during traversal. Uses the index returned by add_relationship
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
                stats.rels += 2;
            }
        },
    )?;

    // Organisations (Company/University) + Person workAt Company + Person studyAt
    // University (classYear stored as an rel property), for Q20.
    let mut org: HashMap<i64, u32> = HashMap::new();
    for_each_row(
        &static_.join("Organisation"),
        &["id", "type", "name", "LocationPlaceId"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &[v[1]]).unwrap(); // label = Company / University
                builder.set_prop_str(id, "name", v[2]).unwrap();
                org.insert(lid, id);
                // Organisation -[orgPlace]-> Place (City/Country), for IC11.
                if let Some(&place_node) = v[3].parse::<i64>().ok().and_then(|i| place.get(&i)) {
                    builder
                        .add_relationship(id, place_node, "orgPlace")
                        .unwrap();
                    stats.rels += 1;
                }
            }
        },
    )?;
    for_each_row(
        &dynamic.join("Person_workAt_Company"),
        &["PersonId", "CompanyId", "workFrom"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let c = v[1].parse::<i64>().ok().and_then(|i| org.get(&i));
            if let (Some(&p), Some(&c)) = (p, c) {
                let idx = builder.add_relationship(p, c, "workAt").unwrap();
                builder.set_relationship_props_by_index(
                    idx,
                    &[("wf", PropertyValue::Integer(v[2].parse().unwrap_or(0)))],
                ); // IC11
                stats.rels += 1;
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
                stats.rels += 1;
            }
        },
    )?;

    Ok((builder.finalize(None), stats))
}
