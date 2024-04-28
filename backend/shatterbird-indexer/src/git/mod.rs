use std::collections::HashMap;
use std::path::Path;

use bson::doc;
use eyre::eyre;
use futures::{FutureExt, StreamExt};
use gix::object::Kind;
use gix::traverse::tree::Visit;
use gix::{ObjectId, Repository};
use serde::Serialize;
use tracing::{debug, info, instrument, warn};

use shatterbird_storage::model::{BlobFile, Commit, FileContent, Line, Node};
use shatterbird_storage::{Id, Model, Storage};

struct Walker<'s, 'r> {
    storage: &'s Storage,
    repo: &'r Repository,
}

impl<'s, 'r> Walker<'_, 'r> {
    #[instrument(skip_all, fields(tree = %tree.id), err)]
    async fn visit_tree(&self, tree: gix::Tree<'r>) -> eyre::Result<Id<Node>> {
        if let Some(x) = self.storage.get_by_oid::<Node>(tree.id).await? {
            debug!("skipping existing tree");
            return Ok(x.id());
        }
        let data = tree.decode()?;
        let mut children = HashMap::new();
        for entry in data.entries {
            let child = self.repo.find_object(entry.oid)?;
            let child_id =
                match child.kind {
                    Kind::Tree => {
                        self.visit_tree(child.try_into().map_err(|o: gix::Object| {
                            eyre!("can't read object {} as a tree", o.id)
                        })?)
                        .boxed_local()
                        .await?
                    }
                    Kind::Blob => {
                        self.visit_blob(child.try_into().map_err(|o: gix::Object| {
                            eyre!("can't read object {} as a blob", o.id)
                        })?)
                        .await?
                    }
                    _ => {
                        return Err(eyre!(
                            "object {} is a {}, not a file or directory",
                            child.id,
                            child.kind
                        ))
                    }
                };
            children.insert(entry.filename.to_string(), child_id);
        }
        let result = Node {
            _id: Id::new(),
            oid: tree.id,
            content: FileContent::Directory { children },
        };
        debug!("saving tree as {}", result.id());
        self.storage.insert_one(&result).await?;
        Ok(result.id())
    }

    fn try_parse_lines<'d>(&self, data: &'d [u8]) -> eyre::Result<Vec<&'d str>> {
        let string = std::str::from_utf8(data)?;
        let mut result = Vec::new();
        for ln in string.lines() {
            eyre::ensure!(ln.len() < 10_000);
            result.push(ln);
        }
        Ok(result)
    }

    #[instrument(skip_all, fields(blob = %blob.id), err)]
    async fn visit_blob(&self, blob: gix::Blob<'r>) -> eyre::Result<Id<Node>> {
        if let Some(x) = self.storage.get_by_oid::<Node>(blob.id).await? {
            debug!("skipping existing blob");
            return Ok(x.id());
        }
        let content = match self.try_parse_lines(&blob.data) {
            Ok(lines) => {
                let lines: Vec<_> = lines
                    .iter()
                    .map(|ln| Line {
                        _id: Id::new(),
                        text: ln.to_string(),
                    })
                    .collect();
                self.storage.insert_many(lines.iter()).await?;
                FileContent::Text {
                    size: blob.data.len() as _,
                    lines: lines.into_iter().map(|ln| ln.id()).collect(),
                }
            }
            Err(_) => {
                let file = BlobFile {
                    _id: Id::new(),
                    data: blob.data[..10_000].to_vec(),
                };
                self.storage.insert_one(&file).await?;
                FileContent::Blob {
                    size: blob.data.len() as _,
                    content: file.id(),
                }
            }
        };
        let result = Node {
            _id: Id::new(),
            oid: blob.id,
            content,
        };
        debug!("saving blob as {}", result.id());
        self.storage.insert_one(&result).await?;
        Ok(result.id())
    }
}

pub async fn index(storage: &Storage, root: &Path) -> eyre::Result<()> {
    let repo = gix::open(root)?;
    let mut head = repo.head()?;
    let commit = head.peel_to_commit_in_place()?;
    let commit_info = commit.decode()?;
    let tree = commit.tree()?;
    
    if let Some(_) = storage.get_by_oid::<Commit>(tree.id).await? {
        info!("skipping existing commit");
        return Ok(());
    }

    let indexer = Walker {
        storage,
        repo: &repo,
    };
    let root = indexer.visit_tree(tree).await?;

    let mut parents = Vec::new();
    for parent in commit_info.parents() {
        let found = storage.get_by_oid::<Commit>(parent).await?;
        let found = match found {
            Some(x) => x,
            None => {
                warn!("parent {:?} is not found in db, ignoring", parent);
                continue;
            }
        };
        parents.push(found.id())
    }

    let commit = Commit {
        _id: Id::new(),
        oid: commit.id,
        root,
        parents,
    };
    storage.insert_one(&commit).await?;

    Ok(())
}
