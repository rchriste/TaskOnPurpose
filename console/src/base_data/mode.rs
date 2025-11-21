use surrealdb::{opt::RecordId, sql::Thing};

use crate::{
    data_storage::surrealdb_layer::{
        surreal_item::{SurrealModeScope, SurrealUrgency, SurrealUrgencyNoData},
        surreal_mode::{SurrealMode, SurrealScope},
    },
    node::{
        Filter, item_node::ItemNode,
        why_in_scope_and_action_with_item_status::WhyInScopeAndActionWithItemStatus,
    },
};

#[derive(Debug)]
pub(crate) struct Mode<'s> {
    surreal_mode: &'s SurrealMode,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum ModeCategory<'e> {
    Core,
    NonCore,
    OutOfScope,
    NotDeclared { item_to_specify: &'e RecordId },
}

impl<'s> Mode<'s> {
    pub(crate) fn new(surreal_mode: &'s SurrealMode) -> Self {
        Self { surreal_mode }
    }

    pub(crate) fn get_name(&self) -> &'s str {
        &self.surreal_mode.summary
    }

    pub(crate) fn get_parent(&self) -> &'s Option<Thing> {
        &self.surreal_mode.parent_mode
    }

    pub(crate) fn get_surreal_id(&self) -> &'s Thing {
        self.surreal_mode
            .id
            .as_ref()
            .expect("Comes from the database so this is always present")
    }

    pub(crate) fn get_core_in_scope(&self) -> &[SurrealScope] {
        &self.surreal_mode.core_in_scope
    }

    pub(crate) fn get_non_core_in_scope(&self) -> &[SurrealScope] {
        &self.surreal_mode.non_core_in_scope
    }

    pub(crate) fn get_explicitly_out_of_scope_items(&self) -> &[Thing] {
        &self.surreal_mode.explicitly_out_of_scope_items
    }

    pub(crate) fn get_category_by_importance<'a>(
        &self,
        item: &'a ItemNode<'a>,
    ) -> ModeCategory<'a> {
        if item.has_parents(Filter::Active) {
            let mode_categories = item.get_immediate_parents(Filter::Active).filter_map(|x| {
            match x.get_importance_scope() {
                Some(importance_scope) => match importance_scope {
                    SurrealModeScope::AllModes => Some(ModeCategory::NonCore),
                    SurrealModeScope::DefaultModesWithChanges { extra_modes_included } => todo!("Need to check default modes, and extra modes, and if extra_modes_included causes it to get pulled in"),
                },
                None => Some(ModeCategory::OutOfScope),
            }
        } ).collect::<Vec<_>>();
            mode_categories.select_highest_mode_category()
        } else if self
            .surreal_mode
            .core_in_scope
            .iter()
            .any(|x| x.is_importance_in_scope && x.for_item == *item.get_surreal_record_id())
        {
            ModeCategory::Core
        } else if self
            .surreal_mode
            .non_core_in_scope
            .iter()
            .any(|x| x.is_importance_in_scope && x.for_item == *item.get_surreal_record_id())
        {
            ModeCategory::NonCore
        } else if self
            .surreal_mode
            .explicitly_out_of_scope_items
            .iter()
            .any(|x| x == item.get_surreal_record_id())
        {
            ModeCategory::OutOfScope
        } else {
            ModeCategory::NotDeclared {
                item_to_specify: item.get_surreal_record_id(),
            }
        }
    }

    pub(crate) fn get_category_by_urgency<'a>(
        &self,
        item: &'a WhyInScopeAndActionWithItemStatus<'a>,
    ) -> ModeCategory<'a> {
        match item.get_urgency_now() {
            Some(urgency) => {
                // Delegate to the shared helper so importance- and urgency-based
                // categorization use the same parent-chain and scope logic.
                let item_node = item.get_action().get_item_node();
                self.get_category_by_urgency_for_item_node_with_urgency(item_node, &urgency)
            }
            None => ModeCategory::NotDeclared {
                item_to_specify: item.get_surreal_record_id(),
            },
        }
    }

    /// The idea is that ItemNode assumes that the Action is to MakeProgress so this function should be called rather than
    /// get_category_by_urgency if you want to assume that the action is to MakeProgress on the item.
    pub(crate) fn get_category_by_urgency_for_item_node<'a>(
        &self,
        item: &'a ItemNode<'a>,
    ) -> ModeCategory<'a> {
        match item.get_urgency_now() {
            Some(Some(urgency)) => {
                self.get_category_by_urgency_for_item_node_with_urgency(item, &urgency)
            }
            Some(None) | None => ModeCategory::NotDeclared {
                item_to_specify: item.get_surreal_record_id(),
            },
        }
    }

    pub(crate) fn is_in_scope_any(&self, items: &[&ItemNode<'_>]) -> bool {
        items
            .iter()
            .any(|x| match self.get_category_by_importance(x) {
                ModeCategory::Core => true,
                ModeCategory::NonCore => true,
                ModeCategory::OutOfScope | ModeCategory::NotDeclared { .. } => {
                    match self.get_category_by_urgency_for_item_node(x) {
                        ModeCategory::Core => true,
                        ModeCategory::NonCore => true,
                        ModeCategory::OutOfScope | ModeCategory::NotDeclared { .. } => false,
                    }
                }
            })
    }

    /// Shared helper for urgency-based categorization that:
    /// - Walks the item and its parents (most-specific first)
    /// - Looks at this mode's `core_in_scope`, `non_core_in_scope`, and
    ///   `explicitly_out_of_scope_items` lists
    /// - Uses the following precedence *within the same item*:
    ///   Core > NonCore > OutOfScope
    /// - Uses the following precedence *across the parent chain*:
    ///   the most specific item (closest to the current item) that has a
    ///   setting wins
    /// - Only if no level specifies anything do we fall back to:
    ///   - NonCore when the urgency scope is `AllModes`
    ///   - NotDeclared otherwise
    fn get_category_by_urgency_for_item_node_with_urgency<'a>(
        &self,
        item_node: &'a ItemNode<'a>,
        urgency: &SurrealUrgency,
    ) -> ModeCategory<'a> {
        // Helper to see if a SurrealScope entry applies for this urgency.
        let urgency_matches = |scope_urgency: &SurrealUrgencyNoData| -> bool {
            match (scope_urgency, urgency) {
                (SurrealUrgencyNoData::CrisesUrgent, SurrealUrgency::CrisesUrgent(_)) => true,
                (SurrealUrgencyNoData::Scheduled, SurrealUrgency::Scheduled(_, _)) => true,
                (SurrealUrgencyNoData::DefinitelyUrgent, SurrealUrgency::DefinitelyUrgent(_)) => {
                    true
                }
                (SurrealUrgencyNoData::MaybeUrgent, SurrealUrgency::MaybeUrgent(_)) => true,
                _ => false,
            }
        };

        // Start from the current item and walk up the parent chain,
        // letting more specific settings override parent settings.
        let self_and_parents = item_node.get_self_and_parents(Filter::Active);

        for item in self_and_parents.iter().rev() {
            let item_id = item.get_surreal_record_id();

            // Track the best category for THIS specific item, based on precedence:
            // Core > NonCore > OutOfScope.
            let mut category_for_this_item: Option<ModeCategory> = None;

            // Explicitly out-of-scope for this mode and this item.
            if self
                .surreal_mode
                .explicitly_out_of_scope_items
                .iter()
                .any(|x| x == item_id)
            {
                category_for_this_item = Some(ModeCategory::OutOfScope);
            }

            // Non-core scopes for this mode, only for this item and this urgency.
            if self.surreal_mode.non_core_in_scope.iter().any(|scope| {
                scope.for_item == *item_id
                    && scope
                        .urgencies_to_include
                        .iter()
                        .any(|u| urgency_matches(u))
            }) {
                category_for_this_item = Some(ModeCategory::NonCore);
            }

            // Core scopes for this mode, only for this item and this urgency.
            if self.surreal_mode.core_in_scope.iter().any(|scope| {
                scope.for_item == *item_id
                    && scope
                        .urgencies_to_include
                        .iter()
                        .any(|u| urgency_matches(u))
            }) {
                category_for_this_item = Some(ModeCategory::Core);
            }

            // If this item has any explicit setting, it wins over all parents. So this is why we are short circuiting the loop and exiting on the first match
            if let Some(category) = category_for_this_item {
                return category;
            }
        }

        // No explicit scope found on the item or any of its parents. At this
        // point, the only thing we know is the urgency's mode scope:
        match urgency.get_scope() {
            // AllModes: in scope for all modes, but as NonCore by default.
            SurrealModeScope::AllModes => ModeCategory::NonCore,
            // DefaultModesWithChanges: only in scope where explicitly configured,
            // so when nothing is configured for this mode we treat it as NotDeclared.
            SurrealModeScope::DefaultModesWithChanges { .. } => ModeCategory::NotDeclared {
                item_to_specify: item_node.get_surreal_record_id(),
            },
        }
    }
}

trait SelectHighestModeCategory<'t> {
    fn select_highest_mode_category(self) -> ModeCategory<'t>;
}

impl<'a> SelectHighestModeCategory<'a> for Vec<ModeCategory<'a>> {
    fn select_highest_mode_category(self) -> ModeCategory<'a> {
        if self.iter().any(|x| x == &ModeCategory::Core) {
            ModeCategory::Core
        } else if self.iter().any(|x| x == &ModeCategory::NonCore) {
            ModeCategory::NonCore
        } else if self.iter().any(|x| x == &ModeCategory::OutOfScope) {
            ModeCategory::OutOfScope
        } else if let Some(item_to_specify) = self.into_iter().find_map(|x| match x {
            ModeCategory::NotDeclared { item_to_specify } => Some(item_to_specify),
            _ => None,
        }) {
            ModeCategory::NotDeclared { item_to_specify }
        } else {
            panic!(
                "This should not happen, because we are getting self and parents so there should always be a ModeCategory match"
            )
        }
    }
}
