use eyre::{eyre, OptionExt, Report};
use futures::join;
use lsp_types::{Position, Range};
use mongodb::bson::doc;

use shatterbird_storage::model::lang::{EdgeInfoDiscriminants, VertexInfo};
use shatterbird_storage::model::{Edge, Vertex};
use shatterbird_storage::{util, Storage};

use crate::language_server::error::LspError;

pub async fn find(
    storage: &Storage,
    req: &lsp_types::GotoDefinitionParams,
) -> Result<Option<lsp_types::GotoDefinitionResponse>, LspError> {
    // range --(Definition)-> DefinitionResult
    let found = util::graph::find(
        storage,
        Some(EdgeInfoDiscriminants::Item),
        &req.text_document_position_params,
        false,
    )
    .await?;
    let results = found
        .found
        .into_iter()
        .filter_map(|v| match v.data {
            VertexInfo::DefinitionResult {} => Some(v.id),
            _ => None,
        })
        .collect::<Vec<_>>();

    // DefinitionResult <--(Item)-- Range
    let items = storage
        .find::<Edge>(
            doc! {
                "data.out_v": { "$in": results },
                "data.edge": { "$eq": "Item" }
            },
            None,
        )
        .await?
        .into_iter()
        .flat_map(|e| e.data.in_vs().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let definitions = storage
        .find::<Vertex>(doc! { "_id": { "$in": items} }, None)
        .await?;

    let mut locations = Vec::new();
    for def in definitions {
        let def = match def.data {
            VertexInfo::Range { range, .. } => range,
            _ => {
                return Err(LspError::Internal(eyre!(
                    "expected range, found {:#?}",
                    def.data
                )))
            }
        };
        let range = storage
            .get(def)
            .await?
            .ok_or_eyre(eyre!("range {} referenced, but not found", def))?;
        let path = async {
            let path = util::graph::find_file_path(storage, &range).await?;
            format!("bird:///{}", path.join("/")).parse().map_err(Report::new)
        };
        let line_no = util::graph::find_line_no(storage, &range);
        let (path, line_no) = join!(path, line_no);
        let (path, line_no) = (path?, line_no?);
        locations.push(lsp_types::Location {
            uri: path,
            range: Range {
                start: Position::new(line_no as _, range.start),
                end: Position::new(line_no as _, range.end),
            },
        })
    }
    Ok(Some(lsp_types::GotoDefinitionResponse::Array(locations)))
}
