use surrealdb::sql::Thing;

use crate::{
    base_data::mode::{Mode, ModeCategory},
    data_storage::surrealdb_layer::surreal_mode::SurrealMode,
};

use super::{
    item_node::ItemNode,
    why_in_scope_and_action_with_item_status::WhyInScopeAndActionWithItemStatus,
};

#[derive(Debug)]
pub(crate) struct ModeNode<'s> {
    mode: &'s Mode<'s>,
    parent: Option<Box<ModeNode<'s>>>,
}

impl PartialEq for ModeNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.get_surreal_id() == other.get_surreal_id()
    }
}

impl Eq for ModeNode<'_> {}

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

    pub(crate) fn create_self_parent_chain(&self) -> Vec<&'s Mode<'s>> {
        let mut chain = vec![];
        chain.push(self.mode);
        if let Some(ref parent) = self.parent {
            chain.extend(parent.create_self_parent_chain());
        }
        chain
    }

    pub(crate) fn get_surreal_id(&self) -> &Thing {
        self.mode.get_surreal_id()
    }

    pub(crate) fn get_surreal(&self) -> &SurrealMode {
        self.mode.get_surreal()
    }

    pub(crate) fn get_mode(&self) -> &Mode<'s> {
        self.mode
    }

    pub(crate) fn get_name(&self) -> &str {
        self.mode.get_name()
    }

    pub(crate) fn get_category_by_importance<'a>(
        &self,
        item: &'a ItemNode<'a>,
    ) -> ModeCategory<'a> {
        self.mode.get_category_by_importance(item)
    }

    pub(crate) fn get_category_by_urgency<'a>(
        &self,
        item: &'a WhyInScopeAndActionWithItemStatus<'a>,
    ) -> ModeCategory<'a> {
        self.mode.get_category_by_urgency(item)
    }

    pub(crate) fn is_in_scope_any(&self, items: &[&ItemNode<'_>]) -> bool {
        self.mode.is_in_scope_any(items)
    }
}
