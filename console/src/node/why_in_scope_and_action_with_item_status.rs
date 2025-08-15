use std::{hash::Hash, iter::once};

use ahash::HashSet;
use surrealdb::sql::Thing;

use crate::data_storage::surrealdb_layer::{
    surreal_in_the_moment_priority::SurrealAction, surreal_time_spent::SurrealWhyInScope,
};

use super::{SurrealUrgency, action_with_item_status::ActionWithItemStatus, item_node::ItemNode};

#[derive(Debug, Clone)]
pub(crate) struct WhyInScopeAndActionWithItemStatus<'s> {
    why_in_scope: HashSet<WhyInScope>,
    action: ActionWithItemStatus<'s>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum WhyInScope {
    Importance,
    Urgency,
    MenuNavigation,
}

impl From<SurrealWhyInScope> for WhyInScope {
    fn from(surreal: SurrealWhyInScope) -> Self {
        match surreal {
            SurrealWhyInScope::Importance => WhyInScope::Importance,
            SurrealWhyInScope::Urgency => WhyInScope::Urgency,
            SurrealWhyInScope::MenuNavigation => WhyInScope::MenuNavigation,
        }
    }
}

pub(crate) trait ToSurreal<T> {
    fn to_surreal(&self) -> T;
}

impl ToSurreal<SurrealWhyInScope> for WhyInScope {
    fn to_surreal(&self) -> SurrealWhyInScope {
        match self {
            WhyInScope::Importance => SurrealWhyInScope::Importance,
            WhyInScope::Urgency => SurrealWhyInScope::Urgency,
            WhyInScope::MenuNavigation => SurrealWhyInScope::MenuNavigation,
        }
    }
}

impl ToSurreal<SurrealWhyInScope> for &WhyInScope {
    fn to_surreal(&self) -> SurrealWhyInScope {
        match self {
            WhyInScope::Importance => SurrealWhyInScope::Importance,
            WhyInScope::Urgency => SurrealWhyInScope::Urgency,
            WhyInScope::MenuNavigation => SurrealWhyInScope::MenuNavigation,
        }
    }
}

impl ToSurreal<Vec<SurrealWhyInScope>> for HashSet<WhyInScope> {
    fn to_surreal(&self) -> Vec<SurrealWhyInScope> {
        self.iter().map(|x| x.to_surreal()).collect()
    }
}

impl ToSurreal<Vec<SurrealWhyInScope>> for &HashSet<WhyInScope> {
    fn to_surreal(&self) -> Vec<SurrealWhyInScope> {
        self.iter().map(|x| x.to_surreal()).collect()
    }
}

impl WhyInScope {
    pub(crate) fn new_menu_navigation() -> HashSet<Self> {
        once(WhyInScope::MenuNavigation).collect()
    }
}

impl PartialEq for WhyInScopeAndActionWithItemStatus<'_> {
    fn eq(&self, other: &Self) -> bool {
        //Skip testing why_in_scope, it should be the same so same objects can be joined
        self.get_action() == other.get_action()
    }
}

impl Eq for WhyInScopeAndActionWithItemStatus<'_> {}

impl Hash for WhyInScopeAndActionWithItemStatus<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        //Skip putting why_in_scope in the hash, it should be the same so same objects can be joined
        self.get_action().hash(state);
    }
}

impl<'e> WhyInScopeAndActionWithItemStatus<'e> {
    pub(crate) fn new(why_in_scope: HashSet<WhyInScope>, action: ActionWithItemStatus<'e>) -> Self {
        Self {
            why_in_scope,
            action,
        }
    }

    pub(crate) fn is_in_scope_for_importance(&self) -> bool {
        self.why_in_scope
            .iter()
            .any(|x| x == &WhyInScope::Importance)
    }

    pub(crate) fn get_urgency_now(&self) -> SurrealUrgency {
        self.action.get_urgency_now()
    }

    pub(crate) fn get_action(&self) -> &ActionWithItemStatus<'e> {
        &self.action
    }

    pub(crate) fn extend_why_in_scope(&mut self, why_in_scope: &HashSet<WhyInScope>) {
        self.why_in_scope.extend(why_in_scope.clone());
    }

    pub(crate) fn get_why_in_scope(&self) -> &HashSet<WhyInScope> {
        &self.why_in_scope
    }

    pub(crate) fn get_surreal_record_id(&self) -> &Thing {
        self.action.get_surreal_record_id()
    }

    pub(crate) fn clone_to_surreal_action(&self) -> SurrealAction {
        self.action.clone_to_surreal_action()
    }

    pub(crate) fn get_item_node(&self) -> &ItemNode<'e> {
        self.action.get_item_node()
    }
}
