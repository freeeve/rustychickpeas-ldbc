//! RDF -> property-graph loader.
//!
//! Maps parsed N-Triples into a rustychickpeas [`GraphSnapshot`] with the
//! standard RDF-as-property-graph convention:
//!   * every IRI/blank subject or IRI object becomes a node (one per resource);
//!   * `rdf:type` makes the object's local name a **label** on the subject;
//!   * a predicate with an IRI/blank object becomes a typed **edge**
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

use crate::harness::Result;
use super::ntriples::{self, Term};

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Counts from a load, for the run banner.
#[derive(Debug, Default)]
pub struct SpbStats {
    pub resources: usize,
    pub triples: usize,
    pub edges: usize,
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

    // Pass 1: assign a node id to every resource, collect rdf:type labels, and
    // remember each IRI to store as a `uri` property (the cross-engine join key).
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut labels: HashMap<u32, Vec<String>> = HashMap::new();
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
        let sid = intern(&t.subject, &mut ids, &mut uri_of, &mut next);
        if predicate_is(t, RDF_TYPE) {
            if let Term::Iri(class) = &t.object {
                labels.entry(sid).or_default().push(ntriples::local_name(class).to_string());
            }
        } else if t.object.is_resource() {
            intern(&t.object, &mut ids, &mut uri_of, &mut next);
        }
    }

    // Pass 2: create nodes (in id order) with their labels and uri.
    let mut builder = GraphBuilder::new(Some(next as usize), Some(triples.len()));
    for id in 0..next {
        let node_labels: Vec<&str> = labels.get(&id).map(|v| v.iter().map(String::as_str).collect()).unwrap_or_default();
        builder.add_node(Some(id), &node_labels).expect("add_node");
        if let Some(iri) = uri_of.get(&id) {
            builder.set_prop_str(id, "uri", iri).ok();
        }
    }

    // Pass 3: edges (IRI objects) and properties (literal objects).
    let mut stats = SpbStats {
        resources: next as usize,
        triples: triples.len(),
        ..Default::default()
    };
    let mut seen_props: HashSet<(u32, String)> = HashSet::new();
    for t in &triples {
        if predicate_is(t, RDF_TYPE) {
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
                builder.add_relationship(subj, dst, key).expect("add_relationship");
                stats.edges += 1;
            }
            // First literal for a (node, key) wins; the guard both dedups and
            // records the (node, key) so later duplicates fall through.
            Term::Literal { value, datatype, .. } if seen_props.insert((subj, key.to_string())) => {
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
    fn maps_types_to_labels_and_edges_and_props() {
        let (g, stats) = load_str(DOC);
        assert_eq!(stats.resources, 2); // cw1, London
        assert_eq!(stats.edges, 1); // about
        assert!(stats.literals >= 3); // title, lat, views

        let cw = g.nodes_with_label("CreativeWork").unwrap();
        assert_eq!(cw.len(), 1);
        let place = g.nodes_with_label("Place").unwrap();
        assert_eq!(place.len(), 1);

        // Typed literals land as their native column types.
        let london = place.iter().next().unwrap();
        assert_eq!(g.prop(london, "lat").and_then(|v| v.to_f64()), Some(51.5));
        assert!(matches!(
            g.prop(london, "views"),
            Some(rustychickpeas_core::ValueId::I64(42))
        ));
    }

    #[test]
    fn edge_uses_predicate_local_name() {
        let (g, _) = load_str(DOC);
        let cw = g.nodes_with_label("CreativeWork").unwrap().iter().next().unwrap();
        let about: Vec<u32> = g
            .neighbors_by_type(cw, rustychickpeas_core::Direction::Outgoing, "about")
            .collect();
        assert_eq!(about.len(), 1);
    }
}
