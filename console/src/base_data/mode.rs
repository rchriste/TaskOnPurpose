use surrealdb::{opt::RecordId, sql::Thing};

use crate::{
    data_storage::surrealdb_layer::{
        surreal_item::{SurrealModeScope, SurrealUrgency},
        surreal_mode::SurrealMode,
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

#[derive(PartialEq, Eq)]
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

    pub(crate) fn get_surreal(&self) -> &'s SurrealMode {
        self.surreal_mode
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
        } else {
            if self
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
    }

    pub(crate) fn get_category_by_urgency<'a>(
        &self,
        item: &'a WhyInScopeAndActionWithItemStatus<'a>,
    ) -> ModeCategory<'a> {
        match item.get_urgency_now() {
            Some(urgency) => match urgency {
                SurrealUrgency::CrisesUrgent(surreal_mode_scope) => todo!(),
                SurrealUrgency::Scheduled(surreal_mode_scope, surreal_scheduled) => todo!(),
                SurrealUrgency::DefinitelyUrgent(surreal_mode_scope) => match surreal_mode_scope {
                    SurrealModeScope::AllModes => ModeCategory::NonCore,
                    SurrealModeScope::DefaultModesWithChanges {
                        extra_modes_included,
                    } => todo!(
                        "Need to check default modes, and extra modes, and if extra_modes_included causes it to get pulled in"
                    ),
                },
                SurrealUrgency::MaybeUrgent(surreal_mode_scope) => todo!(),
            },
            None => todo!("none"),
        }
    }

    /// The idea is that ItemNode assumes that the Action is to MakeProgress so this function should be called rather than
    /// get_category_by_urgency if you want to assume that the action is to MakeProgress on the item.
    pub(crate) fn get_category_by_urgency_for_item_node<'a>(
        &self,
        item: &'a ItemNode<'a>,
    ) -> ModeCategory<'a> {
        match item.get_urgency_now() {
            Some(Some(urgency)) => match urgency {
                SurrealUrgency::CrisesUrgent(surreal_mode_scope) => todo!(),
                SurrealUrgency::Scheduled(surreal_mode_scope, surreal_scheduled) => todo!(),
                SurrealUrgency::DefinitelyUrgent(surreal_mode_scope) => match surreal_mode_scope {
                    SurrealModeScope::AllModes => ModeCategory::NonCore,
                    SurrealModeScope::DefaultModesWithChanges {
                        extra_modes_included,
                    } => todo!(
                        "Need to check default modes, and extra modes, and if extra_modes_included causes it to get pulled in"
                    ),
                },
                SurrealUrgency::MaybeUrgent(surreal_mode_scope) => todo!(),
            },
            Some(None) => todo!("Some(None), probably the same as just None"),
            None => todo!("none"),
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
            ModeCategory::NotDeclared {
                item_to_specify: item_to_specify,
            }
        } else {
            panic!(
                "This should not happen, because we are getting self and parents so there should always be a ModeCategory match"
            )
        }
    }
}
