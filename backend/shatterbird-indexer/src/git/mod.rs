use std::collections::HashMap;
use std::path::Path;

use eyre::eyre;
use futures::FutureExt;
use gix::object::Kind;
use gix::{Repository};
use tracing::{debug, instrument, warn};

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
            let child_id = match child.kind {
                Kind::Tree => {
                    self.visit_tree(child.try_into_tree()?)
                        .boxed_local()
                        .await?
                }
                Kind::Blob => self.visit_blob(child.try_into_blob()?).await?,
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
            id: Id::new(),
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
                        id: Id::new(),
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
                    id: Id::new(),
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
            id: Id::new(),
            oid: blob.id,
            content,
        };
        debug!("saving blob as {}", result.id());
        self.storage.insert_one(&result).await?;
        Ok(result.id())
    }

    #[instrument(skip_all, fields(commit = %commit.id), err)]
    async fn visit_commit(
        &self,
        commit: gix::Commit<'r>,
        max_depth: u32,
    ) -> eyre::Result<Id<Commit>> {
        let tree = commit.tree()?;
        let commit_info = commit.decode()?;

        let mut parents = Vec::new();
        if max_depth == 0 {
            for parent in commit_info.parents() {
                let found = self.storage.get_by_oid::<Commit>(parent).await?;
                let found = match found {
                    Some(x) => x.id,
                    None => {
                        warn!("parent {:?} is not found in db, ignoring", parent);
                        continue;
                    }
                };
                parents.push(found)
            }
        } else {
            for parent in commit_info.parents() {
                let commit = self.repo.find_object(parent)?.try_into_commit()?;
                parents.push(
                    self.visit_commit(commit, max_depth - 1)
                        .boxed_local()
                        .await?,
                );
            }
        }

        if let Some(x) = self.storage.get_by_oid::<Commit>(commit.id).await? {
            debug!("skipping existing commit");
            return Ok(x.id());
        }

        let commit = Commit {
            id: Id::new(),
            oid: commit.id,
            root: self.visit_tree(tree).await?,
            parents,
        };
        self.storage.insert_one(&commit).await?;
        Ok(commit.id)
    }
}

pub async fn index(storage: &Storage, root: &Path, max_depth: u32) -> eyre::Result<()> {
    let repo = gix::open(root)?;
    let mut head = repo.head()?;
    let commit = head.peel_to_commit_in_place()?;

    let indexer = Walker {
        storage,
        repo: &repo,
    };
    indexer.visit_commit(commit, max_depth).await?;

    Ok(())
}
