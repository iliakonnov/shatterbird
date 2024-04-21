use std::collections::HashMap;
use std::sync::Exclusive;

use bumpalo::Bump;
use multimap::MultiMap;
use rayon::prelude::*;

use lsp_types::lsif::{Document, Edge, Element, Entry, Id, Vertex};

use super::lsif_ext::EdgeExtensions;

macro_rules! entry_ref {
    ($name:ident, $func:ident -> $ty:ty, $pat:pat => $val:expr) => {
        #[derive(Copy, Clone)]
        pub struct $name<'a> {
            entry: &'a Entry
        }

        impl<'a> $name<'a> {
            pub const fn new(entry: &'a Entry) -> Option<Self> {
                match &entry.data {
                    #[allow(unused_variables)]
                    $pat => Some(Self {
                        entry
                    }),
                    _ => None
                }
            }

            pub const fn entry(self) -> &'a Entry {
                self.entry
            }

            pub const fn $func(self) -> &'a $ty {
                match &self.entry.data {
                    $pat => $val,
                    _ => unsafe {
                        std::hint::unreachable_unchecked()
                    }
                }
            }
        }
    };
}

entry_ref!(DocumentRef, document -> Document, Element::Vertex(Vertex::Document(doc)) => doc);
entry_ref!(VertexRef, vertex -> Vertex, Element::Vertex(v) => v);
entry_ref!(EdgeRef, edge -> Edge, Element::Edge(e) => e);

pub struct Graph<'a> {
    arena: Exclusive<&'a Bump>,
    vertices: HashMap<Id, VertexRef<'a>>,
    documents: Vec<DocumentRef<'a>>,
    outgoing: MultiMap<Id, EdgeRef<'a>>,
}

impl<'a> Graph<'a> {
    pub fn new(arena: &'a Bump) -> Self {
        Graph {
            arena: Exclusive::new(arena),
            vertices: HashMap::new(),
            documents: Vec::new(),
            outgoing: MultiMap::new(),
        }
    }

    pub fn add(&mut self, entry: Entry) {
        let entry: &'a mut Entry = self.arena.get_mut().alloc(entry);
        let id = match &entry.id {
            Id::Number(num) => Id::Number(*num),
            Id::String(s) => match s.parse() {
                Ok(num) => Id::Number(num),
                Err(_e) => Id::String(s.clone()),
            },
        };
        match &entry.data {
            Element::Vertex(v) => {
                if let Vertex::Document(doc) = v {
                    self.documents.push(DocumentRef::new(entry).unwrap());
                }
                self.vertices.insert(id, VertexRef::new(entry).unwrap());
            }
            Element::Edge(e) => {
                // out_v -> { in_vs }
                e.edge_data().each().for_each(|edge| {
                    self.outgoing.insert(edge.out_v.clone(), EdgeRef::new(entry).unwrap());
                })
            }
        }
    }

    pub fn vertex(&self, id: &Id) -> Option<VertexRef<'a>> {
        self.vertices.get(id).copied()
    }

    pub fn outgoing_from(&self, v: &Id) -> impl IntoParallelIterator<Item = EdgeRef<'_>> {
        self.outgoing
            .get_vec(v)
            .into_par_iter()
            .flat_map(|x| x.par_iter())
            .copied()
    }

    pub fn documents(&self) -> impl IntoParallelIterator<Item = DocumentRef<'_>> {
        self.documents.par_iter().copied()
    }
}
