use std::collections::VecDeque;
use std::path::Path;
use gix::bstr::{BStr, BString, ByteSlice, ByteVec};
use gix::objs::tree;
use gix::traverse::tree::Visit;
use gix::traverse::tree::visit::Action;
use shatterbird_storage::Storage;


#[derive(Debug, Clone, Default)]
struct Traverser {
    path_deque: VecDeque<BString>,
    path: BString,
}

impl Traverser {
    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }

    fn push_element(&mut self, name: &BStr) {
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(name);
    }
}

impl Visit for Traverser {
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every call is matched with push_tracked_path_component");
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        self.pop_element()
    }

    fn visit_tree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        // TODO
        Action::Continue
    }

    fn visit_nontree(&mut self, entry: &tree::EntryRef<'_>) -> Action {
        // TODO
        Action::Continue
    }
}

pub async fn index(storage: &Storage, root: &Path) -> eyre::Result<()> {
    let repo = gix::open(root)?;
    let mut head = repo.head()?;
    let commit = head.peel_to_commit_in_place()?;
    let tree = commit.tree()?;
    
    let mut traverser = Traverser::default();
    tree.traverse().breadthfirst(&mut traverser)?;
    println!("{:#?}", traverser);
    Ok(())
}