//! RDF -> property-graph loader.
//!
//! Maps parsed N-Triples into a rustychickpeas [`GraphSnapshot`] with the
//! standard RDF-as-property-graph convention:
//!   * every IRI/blank subject or IRI object becomes a node (one per resource);
//!   * `rdf:type` makes the object's local name a **label** on the subject;
//!   * a predicate with an IRI/blank object becomes a typed **rel**
//!     (local name of the predicate);
//!   * a predicate with a literal object becomes a node **property** (local name
//!     of the predicate), typed from the literal's `xsd:` datatype.
//!
//! No triple store, no SPARQL — just a serialization mapper feeding the
//! hand-coded SPB queries.

use std::fs;
use std::path::Path;

use hashbrown::{HashMap, HashSet};
use rustychickpeas_core::{GraphBuilder, GraphSnapshot};

use super::ntriples::{self, Term};
use crate::harness::Result;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const RDFS_SUBCLASS: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
const RDFS_SUBPROP: &str = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf";
/// The trivial universal class: never materialized (no query targets it, and its
/// local name `Thing` collides with `coreconcepts:Thing`).
const OWL_THING: &str = "http://www.w3.org/2002/07/owl#Thing";

/// Close a direct super-of map (`x -> {direct supers}`) under transitivity, so
/// each key maps to all its ancestors. Inputs are tiny (an ontology TBox).
fn close_transitively(m: &mut HashMap<String, HashSet<String>>) {
    loop {
        let mut changed = false;
        let keys: Vec<String> = m.keys().cloned().collect();
        for k in keys {
            for d in m[&k].iter().cloned().collect::<Vec<_>>() {
                if let Some(supers) = m.get(&d).cloned() {
                    let set = m.get_mut(&k).unwrap();
                    for s in supers {
                        changed |= set.insert(s);
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Counts from a load, for the run banner.
#[derive(Debug, Default)]
pub struct SpbStats {
    pub resources: usize,
    pub triples: usize,
    pub rels: usize,
    pub literals: usize,
}

/// Load an N-Triples file into a property graph.
pub fn load_ntriples(path: &Path) -> Result<(GraphSnapshot, SpbStats)> {
    let text = fs::read_to_string(path)?;
    Ok(load_str(&text))
}

/// Load N-Triples from a string (the testable core of [`load_ntriples`]).
pub fn load_str(text: &str) -> (GraphSnapshot, SpbStats) {
    let triples: Vec<_> = ntriples::parse(text).collect();

    // TBox: rdfs:subClassOf / rdfs:subPropertyOf, transitively closed, so RDFS
    // forward-chaining can materialize each type's super-classes and each
    // predicate's super-properties (e.g. `about`/`mentions` -> `tag`). The maps
    // are empty when the input carries no ontology, leaving instance loading
    // unchanged.
    let mut subclass: HashMap<String, HashSet<String>> = HashMap::new();
    let mut subprop: HashMap<String, HashSet<String>> = HashMap::new();
    for t in &triples {
        if let (Term::Iri(s), Term::Iri(p), Term::Iri(o)) = (&t.subject, &t.predicate, &t.object) {
            if p == RDFS_SUBCLASS {
                subclass.entry(s.clone()).or_default().insert(o.clone());
            } else if p == RDFS_SUBPROP {
                subprop.entry(s.clone()).or_default().insert(o.clone());
            }
        }
    }
    close_transitively(&mut subclass);
    close_transitively(&mut subprop);

    // Pass 1: assign a node id to every resource, collect each subject's rdf:type
    // IRIs (expanded with super-classes below), and remember each IRI to store as
    // a `uri` property (the cross-engine join key). TBox triples are skipped.
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut types: HashMap<u32, Vec<String>> = HashMap::new();
    let mut uri_of: HashMap<u32, String> = HashMap::new();
    let mut next: u32 = 0;
    let intern = |term: &Term,
                  ids: &mut HashMap<String, u32>,
                  uri_of: &mut HashMap<u32, String>,
                  next: &mut u32|
     -> u32 {
        let key = resource_key(term).expect("resource term");
        if let Some(&id) = ids.get(&key) {
            return id;
        }
        let id = *next;
        *next += 1;
        ids.insert(key, id);
        if let Term::Iri(iri) = term {
            uri_of.insert(id, percent_decode(iri));
        }
        id
    };
    for t in &triples {
        if is_tbox(t) {
            continue;
        }
        let sid = intern(&t.subject, &mut ids, &mut uri_of, &mut next);
        if predicate_is(t, RDF_TYPE) {
            if let Term::Iri(class) = &t.object {
                types.entry(sid).or_default().push(class.clone());
            }
        } else if t.object.is_resource() {
            intern(&t.object, &mut ids, &mut uri_of, &mut next);
        }
    }

    // Pass 2: create nodes (in id order) with their labels (each type's local name
    // plus those of its super-classes, minus the trivial owl:Thing) and uri.
    let mut builder = GraphBuilder::new(Some(next as usize), Some(triples.len()));
    for id in 0..next {
        let mut label_iris: HashSet<&str> = HashSet::new();
        for ty in types.get(&id).map(Vec::as_slice).unwrap_or_default() {
            label_iris.insert(ty);
            if let Some(supers) = subclass.get(ty) {
                label_iris.extend(
                    supers
                        .iter()
                        .map(String::as_str)
                        .filter(|s| *s != OWL_THING),
                );
            }
        }
        let node_labels: Vec<&str> = label_iris
            .iter()
            .map(|iri| ntriples::local_name(iri))
            .collect();
        builder.add_node(Some(id), &node_labels).expect("add_node");
        if let Some(iri) = uri_of.get(&id) {
            builder.set_prop_str(id, "uri", iri).ok();
        }
    }

    // Pass 3: rels (IRI objects, each materialized for the predicate and its
    // super-properties) and properties (literal objects). TBox triples are skipped.
    let mut stats = SpbStats {
        resources: next as usize,
        triples: triples.len(),
        ..Default::default()
    };
    let mut seen_props: HashSet<(u32, String)> = HashSet::new();
    for t in &triples {
        if predicate_is(t, RDF_TYPE) || is_tbox(t) {
            continue;
        }
        let subj = ids[&resource_key(&t.subject).unwrap()];
        let Term::Iri(pred) = &t.predicate else {
            continue;
        };
        let key = ntriples::local_name(pred);
        match &t.object {
            obj if obj.is_resource() => {
                let dst = ids[&resource_key(obj).unwrap()];
                builder
                    .add_relationship(subj, dst, key)
                    .expect("add_relationship");
                stats.rels += 1;
                // RDFS subPropertyOf: the same statement also satisfies every
                // super-property (e.g. an `about`/`mentions` rel is a `tag` rel).
                if let Some(supers) = subprop.get(pred) {
                    for s in supers {
                        builder
                            .add_relationship(subj, dst, ntriples::local_name(s))
                            .expect("add_relationship");
                    }
                }
            }
            // First literal for a (node, key) wins; the guard both dedups and
            // records the (node, key) so later duplicates fall through.
            Term::Literal {
                value, datatype, ..
            } if seen_props.insert((subj, key.to_string())) => {
                set_literal_prop(&mut builder, subj, key, value, datatype.as_deref());
                stats.literals += 1;
            }
            _ => {}
        }
    }

    (builder.finalize(None), stats)
}

/// Resource identity key: namespaced so a blank `x` and an IRI `x` never collide.
/// IRIs are percent-decoded so the same entity written both percent-encoded and as
/// raw UTF-8 interns to one node (see [`percent_decode`]).
fn resource_key(term: &Term) -> Option<String> {
    match term {
        Term::Iri(iri) => Some(format!("I:{}", percent_decode(iri))),
        Term::Blank(b) => Some(format!("B:{b}")),
        Term::Literal { .. } => None,
    }
}

/// Percent-decode an IRI to a canonical form, so an entity referenced both
/// percent-encoded (`Ottoman%E2%80%93Portuguese`) and as raw UTF-8
/// (`Ottoman–Portuguese`) resolves to a single node — matching how a SPARQL store
/// canonicalizes IRIs on load. A `%XX` that is not valid hex, or a decode that is
/// not valid UTF-8, is left verbatim.
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Whether a triple is an RDFS TBox statement (subClassOf / subPropertyOf),
/// consumed for forward-chaining rather than loaded as instance data.
fn is_tbox(t: &ntriples::Triple) -> bool {
    predicate_is(t, RDFS_SUBCLASS) || predicate_is(t, RDFS_SUBPROP)
}

fn predicate_is(t: &ntriples::Triple, iri: &str) -> bool {
    matches!(&t.predicate, Term::Iri(p) if p == iri)
}

/// Store a literal as the most specific property type its `xsd:` datatype
/// allows, falling back to a string.
fn set_literal_prop(b: &mut GraphBuilder, id: u32, key: &str, value: &str, datatype: Option<&str>) {
    if let Some(dt) = datatype {
        match ntriples::local_name(dt) {
            "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
            | "positiveInteger" => {
                if let Ok(v) = value.parse::<i64>() {
                    b.set_prop_i64(id, key, v).ok();
                    return;
                }
            }
            "double" | "float" | "decimal" => {
                if let Ok(v) = value.parse::<f64>() {
                    b.set_prop_f64(id, key, v).ok();
                    return;
                }
            }
            "boolean" => {
                if let Ok(v) = value.parse::<bool>() {
                    b.set_prop_bool(id, key, v).ok();
                    return;
                }
            }
            _ => {}
        }
    }
    b.set_prop_str(id, key, value).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/title> "Hello" .
<http://ex/cw1> <http://bbc/about> <http://ex/London> .
<http://ex/London> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Place> .
<http://ex/London> <http://geo#lat> "51.5"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://ex/London> <http://geo#views> "42"^^<http://www.w3.org/2001/XMLSchema#integer> .
"#;

    #[test]
    fn maps_types_to_labels_and_rels_and_props() {
        let (g, stats) = load_str(DOC);
        assert_eq!(stats.resources, 2); // cw1, London
        assert_eq!(stats.rels, 1); // about
        assert!(stats.literals >= 3); // title, lat, views

        let cw = g.nodes_with_label("CreativeWork").unwrap();
        assert_eq!(cw.len(), 1);
        let place = g.nodes_with_label("Place").unwrap();
        assert_eq!(place.len(), 1);

        // Typed literals land as their native column types.
        let london = place.iter().next().unwrap();
        assert_eq!(g.prop(london, "lat").and_then(|v| v.f64()), Some(51.5));
        assert_eq!(g.prop(london, "views").and_then(|v| v.i64()), Some(42));
    }

    #[test]
    fn rel_uses_predicate_local_name() {
        let (g, _) = load_str(DOC);
        let cw = g
            .nodes_with_label("CreativeWork")
            .unwrap()
            .iter()
            .next()
            .unwrap();
        let about: Vec<u32> = g
            .neighbors_by_type(cw, rustychickpeas_core::Direction::Outgoing, "about")
            .collect();
        assert_eq!(about.len(), 1);
    }

    #[test]
    fn rdfs_forward_chains_subclass_and_subproperty() {
        // TBox (BlogPost <= CreativeWork <= owl:Thing; Company <= Thing; about <=
        // tag) + an instance: a BlogPost `about` a Company.
        const RDFS: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .
<http://bbc/CreativeWork> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://www.w3.org/2002/07/owl#Thing> .
<http://dbo/Company> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .
<http://bbc/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .
<http://ex/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
"#;
        let (g, stats) = load_str(RDFS);
        // TBox triples are not instance data: only cw1 + Acme become nodes.
        assert_eq!(stats.resources, 2);

        // subClassOf: cw1 is a BlogPost AND a CreativeWork (owl:Thing dropped);
        // Acme is a Company AND a Thing.
        let cw1 = g
            .nodes_with_label("BlogPost")
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert!(g.nodes_with_label("CreativeWork").unwrap().contains(cw1));
        let acme = g
            .nodes_with_label("Company")
            .unwrap()
            .iter()
            .next()
            .unwrap();
        let thing = g.nodes_with_label("Thing").unwrap();
        assert!(thing.contains(acme) && !thing.contains(cw1));

        // subPropertyOf: the `about` rel is also a `tag` rel.
        use rustychickpeas_core::Direction;
        let tag: Vec<u32> = g
            .neighbors_by_type(cw1, Direction::Outgoing, "tag")
            .collect();
        assert_eq!(
            tag,
            g.neighbors_by_type(cw1, Direction::Outgoing, "about")
                .collect::<Vec<_>>()
        );
        assert_eq!(tag.len(), 1);
    }
}
