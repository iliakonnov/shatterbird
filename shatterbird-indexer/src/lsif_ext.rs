use std::iter;
use either::Either;
use lsp_types::lsif::{Edge, EdgeData, EdgeDataMultiIn, Id};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(Clone, Copy, Debug)]
pub struct EdgeDataRef<'a> {
    pub in_v: &'a Id,
    pub out_v: &'a Id,
}

#[derive(Clone, Copy, Debug)]
pub enum EitherEdgeData<'a> {
    Single(&'a EdgeData),
    Multi(&'a EdgeDataMultiIn),
}

impl<'a> EitherEdgeData<'a> {
    pub fn par_each(self) -> impl IndexedParallelIterator<Item = EdgeDataRef<'a>> {
        match self {
            EitherEdgeData::Single(EdgeData { in_v, out_v }) => {
                Either::Left(rayon::iter::once(EdgeDataRef { in_v, out_v }))
            }
            EitherEdgeData::Multi(EdgeDataMultiIn { out_v, in_vs }) => {
                Either::Right(in_vs.par_iter().map(|in_v| EdgeDataRef { in_v, out_v }))
            }
        }
    }
    
    pub fn each(self) -> impl Iterator<Item = EdgeDataRef<'a>> {
        match self {
            EitherEdgeData::Single(EdgeData { in_v, out_v }) => {
                Either::Left(iter::once(EdgeDataRef { in_v, out_v }))
            }
            EitherEdgeData::Multi(EdgeDataMultiIn { out_v, in_vs }) => {
                Either::Right(in_vs.iter().map(|in_v| EdgeDataRef { in_v, out_v }))
            }
        }
    }
}

pub trait EdgeExtensions {
    fn edge_data(&self) -> EitherEdgeData;
}

impl EdgeExtensions for Edge {
    fn edge_data(&self) -> EitherEdgeData {
        match self {
            Edge::Contains(x) | Edge::Item(lsp_types::lsif::Item { edge_data: x, .. }) => {
                EitherEdgeData::Multi(x)
            }
            Edge::Moniker(x)
            | Edge::NextMoniker(x)
            | Edge::Next(x)
            | Edge::PackageInformation(x)
            | Edge::Definition(x)
            | Edge::Declaration(x)
            | Edge::Hover(x)
            | Edge::References(x)
            | Edge::Implementation(x)
            | Edge::TypeDefinition(x)
            | Edge::FoldingRange(x)
            | Edge::DocumentLink(x)
            | Edge::DocumentSymbol(x)
            | Edge::Diagnostic(x) => EitherEdgeData::Single(x),
        }
    }
}
