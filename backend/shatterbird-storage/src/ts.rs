use crate::model::lang::{EdgeInfoDiscriminants, VertexInfoDiscriminants};
use crate::model::Vertex;
use std::marker::PhantomData;
use ts_rs::TS;

#[derive(TS)]
#[ts(export)]
pub struct Id<T: TS + ?Sized> {
    #[ts(rename = "$oid")]
    pub id: String,

    #[ts(skip)]
    pub _phantom: PhantomData<fn() -> T>,
}

#[derive(TS)]
pub struct EdgeInfo {
    /// Вид ребра
    #[ts(inline)]
    pub edge: EdgeInfoDiscriminants,

    /// Входящий узел, если один
    #[ts(optional)]
    pub in_v: Option<Id<Vertex>>,

    /// Входящие узлы, если несколько
    #[ts(optional)]
    pub in_vs: Option<Vec<Id<Vertex>>>,

    /// Исходящий узел
    pub out_v: Id<Vertex>,
}

#[derive(TS)]
pub struct VertexInfo {
    /// Вид узла
    #[ts(inline)]
    pub vertex: VertexInfoDiscriminants,
}
