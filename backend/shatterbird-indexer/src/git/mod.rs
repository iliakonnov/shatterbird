use either::Either;
use std::collections::HashMap;
use std::path::Path;

use eyre::{eyre};
use futures::FutureExt;
use gix::bstr::{BString};
use gix::object::Kind;
use gix::{ObjectId, Repository};
use tracing::{debug, debug_span, instrument, warn, Instrument};

use shatterbird_storage::model::{BlobFile, Commit, FileContent, Line, Node};
use shatterbird_storage::{Id, Model, Storage};

struct Walker<'s, 'r> {
    storage: &'s Storage,
    repo: &'r Repository,
    path: RepoPath,
}

#[derive(Default)]
struct RepoPath {
    commits: Vec<ObjectId>,
    path: Vec<BString>,
}

impl<'s, 'r> Walker<'s, 'r> {
    #[instrument(skip_all, fields(tree = %tree.id), err)]
    async fn visit_tree(&mut self, tree: gix::Tree<'r>) -> eyre::Result<Id<Node>> {
        if let Some(x) = self.storage.get_by_oid::<Node>(tree.id).await? {
            debug!("skipping existing tree");
            return Ok(x.id());
        }
        let data = tree.decode()?;
        let mut children = HashMap::new();
        for entry in data.entries {
            self.path.path.push(entry.filename.to_owned());
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
            self.path.path.pop();
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
                let lines = self.insert_lines(lines).await?;
                FileContent::Text {
                    size: blob.data.len() as _,
                    lines,
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

    async fn visit_commit(
        &mut self,
        commit: gix::Commit<'r>,
        max_depth: u32,
    ) -> eyre::Result<Id<Commit>> {
        let span = debug_span!("visit_commit", commit = %commit.id);
        self.path.commits.push(commit.id);

        let (tree, commit_info) = {
            let _guard = span.enter();
            (commit.tree()?, commit.decode()?)
        };

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

        async {
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
            self.path.commits.pop();
            self.storage.insert_one(&commit).await?;
            Ok(commit.id)
        }
        .instrument(span)
        .await
    }

    #[instrument(skip_all, err)]
    async fn insert_lines(&self, lines: Vec<&str>) -> eyre::Result<Vec<Id<Line>>> {
        let mut line_ids: Vec<_> = lines
            .iter()
            .map(|ln| {
                Either::<Line, Id<Line>>::Left(Line {
                    id: Id::new(),
                    text: ln.to_string(),
                })
            })
            .collect();

        // Find previous blob
        let commit = self
            .path
            .commits
            .last()
            .expect("current commit must be set");
        let commit = self.repo.find_object(*commit)?.try_into_commit()?;
        let data = commit.decode()?;
        let mut buf = Vec::new();
        for parent in data.parents() {
            let commit = self.repo.find_object(parent)?.try_into_commit()?;
            let blob = commit
                .tree()?
                .lookup_entry(self.path.path.iter().map(AsRef::<[u8]>::as_ref), &mut buf)?;
            let blob = match blob {
                Some(x) => x,
                None => continue,
            };
            let old_ids = match self.storage.get_by_oid::<Node>(blob.object_id()).await? {
                Some(Node {
                    content: FileContent::Text { lines, .. },
                    ..
                }) => lines,
                _ => continue,
            };
            let blob = match blob.object()?.try_into_blob() {
                Ok(x) => x,
                Err(_) => continue,
            };
            let text = match std::str::from_utf8(&blob.data) {
                Ok(x) => x,
                Err(_) => continue,
            };
            let old_lines = text.lines().collect::<Vec<_>>();
            let diff = similar::capture_diff_slices(
                similar::Algorithm::Patience,
                &old_lines[..],
                &lines[..],
            );
            for op in diff {
                if let similar::DiffOp::Equal {
                    old_index,
                    new_index,
                    len,
                } = op
                {
                    for i in 0..len {
                        line_ids[new_index + i] = Either::Right(old_ids[old_index + i]);
                    }
                }
            }
        }

        self.storage
            .insert_many(line_ids.iter().filter_map(|x| x.as_ref().left()))
            .await?;
        Ok(line_ids
            .into_iter()
            .filter_map(|x| x.map_left(|x| x.id()).either_into())
            .collect())
    }
}

pub async fn index(storage: &Storage, root: &Path, max_depth: u32) -> eyre::Result<()> {
    let repo = gix::open(root)?;
    let mut head = repo.head()?;
    let commit = head.peel_to_commit_in_place()?;

    let mut indexer = Walker {
        storage,
        repo: &repo,
        path: RepoPath::default(),
    };
    let result = indexer.visit_commit(commit, max_depth).await?;

    println!("{}", result);
    Ok(())
}
