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

    // Pass 1: assign a node id to every resource, and collect rdf:type labels.
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut labels: HashMap<u32, Vec<String>> = HashMap::new();
    let mut next: u32 = 0;
    let resource_id = |term: &Term, ids: &mut HashMap<String, u32>, next: &mut u32| -> u32 {
        let key = resource_key(term).expect("resource term");
        *ids.entry(key).or_insert_with(|| {
            let id = *next;
            *next += 1;
            id
        })
    };
    for t in &triples {
        let sid = resource_id(&t.subject, &mut ids, &mut next);
        if predicate_is(t, RDF_TYPE) {
            if let Term::Iri(class) = &t.object {
                labels.entry(sid).or_default().push(ntriples::local_name(class).to_string());
            }
        } else if t.object.is_resource() {
            resource_id(&t.object, &mut ids, &mut next);
        }
    }

    // Pass 2: create nodes (in id order) with their labels.
    let mut builder = GraphBuilder::new(Some(next as usize), Some(triples.len()));
    for id in 0..next {
        let node_labels: Vec<&str> = labels.get(&id).map(|v| v.iter().map(String::as_str).collect()).unwrap_or_default();
        builder.add_node(Some(id), &node_labels).expect("add_node");
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
fn resource_key(term: &Term) -> Option<String> {
    match term {
        Term::Iri(iri) => Some(format!("I:{iri}")),
        Term::Blank(b) => Some(format!("B:{b}")),
        Term::Literal { .. } => None,
    }
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
