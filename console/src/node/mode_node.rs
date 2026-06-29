use surrealdb::RecordId;

use crate::base_data::mode::Mode;

pub(crate) struct ModeNode<'s> {
    mode: &'s Mode<'s>,
    parent: Option<Box<ModeNode<'s>>>,
}

impl<'s> ModeNode<'s> {
    pub(crate) fn new(mode: &'s Mode<'s>, all_modes: &'s [Mode<'s>]) -> Self {
        let parent = match mode.get_parent() {
            Some(parent_id) => {
                let parent = all_modes
                    .iter()
                    .find(|mode| mode.get_surreal_id() == parent_id)
                    .expect("Parent mode must exist");
                Some(Box::new(ModeNode::new(parent, all_modes)))
            }
            None => None,
        };

        Self { mode, parent }
    }

    pub(crate) fn create_parent_chain(&self) -> Vec<&'s Mode<'s>> {
        let mut chain = vec![];
        chain.push(self.mode);
        if let Some(ref parent) = self.parent {
            chain.extend(parent.create_parent_chain());
        }
        chain
    }

    pub(crate) fn get_surreal_id(&self) -> &RecordId {
        self.mode.get_surreal_id()
    }

    pub(crate) fn get_name(&self) -> &str {
        self.mode.get_name()
    }
}
