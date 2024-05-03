use either::Either;
use serde::{Deserialize, Serialize};

use super::files::Range;
use crate::Model;
use mongo_model::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[mongo_model(collection = "vertices")]
pub struct Vertex {
    #[serde(rename = "_id")]
    pub id: Id<Self>,
    pub data: VertexInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[mongo_model(collection = "edges")]
pub struct Edge {
    #[serde(rename = "_id")]
    pub id: Id<Self>,
    pub data: EdgeInfo,
}

// Same as https://docs.rs/lsp-types/latest/lsp_types/lsif/enum.Edge.html
// But with all `Id`s replaced with `Id<Vertex>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "edge")]
pub enum EdgeInfo {
    Contains(EdgeDataMultiIn),
    Moniker(EdgeData),
    NextMoniker(EdgeData),
    Next(EdgeData),
    PackageInformation(EdgeData),
    Item(Item),

    Definition(EdgeData),     // "textDocument/definition"
    Declaration(EdgeData),    // "textDocument/declaration"
    Hover(EdgeData),          // "textDocument/hover"
    References(EdgeData),     // "textDocument/references"
    Implementation(EdgeData), // "textDocument/implementation"
    TypeDefinition(EdgeData), // "textDocument/typeDefinition"
    FoldingRange(EdgeData),   // "textDocument/foldingRange"
    DocumentLink(EdgeData),   // "textDocument/documentLink"
    DocumentSymbol(EdgeData), // "textDocument/documentSymbol"
    Diagnostic(EdgeData),     // "textDocument/diagnostic"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDataMultiIn {
    pub in_vs: Vec<Id<Vertex>>,
    pub out_v: Id<Vertex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeData {
    pub in_v: Id<Vertex>,
    pub out_v: Id<Vertex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub document: Id<Vertex>,
    pub property: Option<lsp_types::lsif::ItemKind>,
    #[serde(flatten)]
    pub edge_data: EdgeDataMultiIn,
}

// Same as https://docs.rs/lsp-types/latest/lsp_types/lsif/enum.Vertex.html
// But with all Ranges replaced with Id<Range> instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "vertex")]
pub enum VertexInfo {
    MetaData(lsp_types::lsif::MetaData),
    Project(lsp_types::lsif::Project),
    Document(lsp_types::lsif::Document),
    Range {
        range: Id<Range>,
        tag: Option<lsp_types::lsif::RangeTag>,
    },
    ResultSet(lsp_types::lsif::ResultSet),
    Moniker(lsp_types::Moniker),
    PackageInformation(lsp_types::lsif::PackageInformation),
    DefinitionResult {},
    DeclarationResult {},
    TypeDefinitionResult {},
    ReferenceResult {},
    ImplementationResult {},
    FoldingRangeResult {
        result: Vec<lsp_types::FoldingRange>,
    },
    HoverResult {
        result: lsp_types::Hover,
    },
    DocumentSymbolResult {
        result: lsp_types::lsif::DocumentSymbolOrRangeBasedVec,
    },
    DocumentLinkResult {
        result: Vec<DocumentLink>,
    },
    DiagnosticResult {
        result: Vec<Diagnostic>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldingRange {
    pub start: Id<Range>,
    pub end: Id<Range>,
    pub kind: Option<lsp_types::FoldingRangeKind>,
    pub collapsed_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentLink {
    pub range: Id<Range>,
    pub target: Option<lsp_types::Url>,
    pub tooltip: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Id<Range>,
    pub severity: Option<lsp_types::DiagnosticSeverity>,
    pub code: Option<lsp_types::NumberOrString>,
    pub code_description: Option<lsp_types::CodeDescription>,
    pub source: Option<String>,
    pub message: String,
    pub related_information: Option<Vec<DiagnosticRelatedInformation>>,
    pub tags: Option<Vec<lsp_types::DiagnosticTag>>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRelatedInformation {
    pub location: Id<Range>,
    pub message: String,
}

impl EdgeInfo {
    pub fn in_vs(&self) -> impl Iterator<Item = Id<Vertex>> + '_ {
        match self {
            EdgeInfo::Contains(x) | EdgeInfo::Item(Item { edge_data: x, .. }) => {
                Either::Right(x.in_vs.iter().copied())
            }
            EdgeInfo::Moniker(x)
            | EdgeInfo::NextMoniker(x)
            | EdgeInfo::Next(x)
            | EdgeInfo::PackageInformation(x)
            | EdgeInfo::Definition(x)
            | EdgeInfo::Declaration(x)
            | EdgeInfo::Hover(x)
            | EdgeInfo::References(x)
            | EdgeInfo::Implementation(x)
            | EdgeInfo::TypeDefinition(x)
            | EdgeInfo::FoldingRange(x)
            | EdgeInfo::DocumentLink(x)
            | EdgeInfo::DocumentSymbol(x)
            | EdgeInfo::Diagnostic(x) => Either::Left(std::iter::once(x.in_v)),
        }
    }
    
    pub fn out_vs(&self) -> impl Iterator<Item = Id<Vertex>> {
        match self {

            EdgeInfo::Contains(x) | EdgeInfo::Item(Item { edge_data: x, .. }) => {
                Either::Right(std::iter::once(x.out_v))
            }
            EdgeInfo::Moniker(x)
            | EdgeInfo::NextMoniker(x)
            | EdgeInfo::Next(x)
            | EdgeInfo::PackageInformation(x)
            | EdgeInfo::Definition(x)
            | EdgeInfo::Declaration(x)
            | EdgeInfo::Hover(x)
            | EdgeInfo::References(x)
            | EdgeInfo::Implementation(x)
            | EdgeInfo::TypeDefinition(x)
            | EdgeInfo::FoldingRange(x)
            | EdgeInfo::DocumentLink(x)
            | EdgeInfo::DocumentSymbol(x)
            | EdgeInfo::Diagnostic(x) => Either::Left(std::iter::once(x.out_v)),
        }
    }
}
