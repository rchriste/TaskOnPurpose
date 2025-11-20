use crate::{
    base_data::mode::ModeCategory,
    node::{
        item_node::ItemNode, mode_node::ModeNode,
        why_in_scope_and_action_with_item_status::WhyInScopeAndActionWithItemStatus,
    },
};

pub(crate) struct CurrentMode<'s> {
    mode: &'s ModeNode<'s>,
}

pub(crate) trait IsInTheMode<'t> {
    fn get_category_by_importance(&self, item_node: &'t ItemNode) -> ModeCategory<'t>;
    fn get_category_by_urgency(
        &self,
        item: &'t WhyInScopeAndActionWithItemStatus,
    ) -> ModeCategory<'t>;
}

impl<'a> IsInTheMode<'a> for &Option<CurrentMode<'a>> {
    fn get_category_by_importance(&self, item_node: &'a ItemNode) -> ModeCategory<'a> {
        match self {
            Some(current_mode) => current_mode.get_category_by_importance(item_node),
            None => ModeCategory::NonCore,
        }
    }

    fn get_category_by_urgency(
        &self,
        item: &'a WhyInScopeAndActionWithItemStatus,
    ) -> ModeCategory<'a> {
        match self {
            Some(current_mode) => current_mode.get_category_by_urgency(item),
            None => ModeCategory::NonCore,
        }
    }
}

impl<'a> IsInTheMode<'a> for CurrentMode<'a> {
    fn get_category_by_importance(&self, item_node: &'a ItemNode<'a>) -> ModeCategory<'a> {
        self.mode.get_category_by_importance(item_node)
    }

    fn get_category_by_urgency(
        &self,
        item: &'a WhyInScopeAndActionWithItemStatus<'a>,
    ) -> ModeCategory<'a> {
        self.mode.get_category_by_urgency(item)
    }
}

impl<'s> CurrentMode<'s> {
    pub(crate) fn new(mode_id: &surrealdb::sql::Thing, mode_nodes: &'s [ModeNode<'s>]) -> Self {
        let mode = mode_nodes
            .iter()
            .find(|mode_node| mode_node.get_surreal_id() == mode_id)
            .expect("Mode must exist");

        CurrentMode { mode }
    }

    pub(crate) fn get_mode(&self) -> &ModeNode<'_> {
        self.mode
    }

    pub(crate) fn get_category_by_importance<'a>(
        &self,
        item: &'a ItemNode<'a>,
    ) -> ModeCategory<'a> {
        self.mode.get_category_by_importance(item)
    }
}
