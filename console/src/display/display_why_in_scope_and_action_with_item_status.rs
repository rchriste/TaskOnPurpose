use std::fmt::{Display, Formatter};

use ahash::HashSet;
use surrealdb::RecordId;

use crate::{
    data_storage::surrealdb_layer::{
        surreal_in_the_moment_priority::SurrealAction, surreal_item::SurrealUrgency,
    },
    display::display_action_with_item_status::DisplayActionWithItemStatus,
    node::{
        Filter,
        action_with_item_status::ActionWithItemStatus,
        why_in_scope_and_action_with_item_status::{WhyInScope, WhyInScopeAndActionWithItemStatus},
    },
};

use super::display_item_node::DisplayFormat;

#[derive(Clone)]
pub(crate) struct DisplayWhyInScopeAndActionWithItemStatus<'s> {
    item: &'s WhyInScopeAndActionWithItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayWhyInScopeAndActionWithItemStatus<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_in_scope_for_importance() {
            write!(f, "🔝 ")?;
        }

        let urgency = self.get_urgency_now();
        match urgency {
            SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled => write!(f, "🚨 ")?,
            SurrealUrgency::MoreUrgentThanMode => write!(f, "🔥 ")?,
            SurrealUrgency::InTheModeByImportance => {}
            SurrealUrgency::InTheModeDefinitelyUrgent => write!(f, "🔴 ")?,
            SurrealUrgency::InTheModeMaybeUrgent => write!(f, "🟡 ")?,
            SurrealUrgency::ScheduledAnyMode(..) => write!(f, "🗓️❗ ")?,
            SurrealUrgency::InTheModeScheduled(..) => write!(f, "🗓️⭳ ")?,
        }

        write!(
            f,
            "{}",
            DisplayActionWithItemStatus::new(self.get_action(), self.filter, self.display_format)
        )
    }
}

impl<'s> DisplayWhyInScopeAndActionWithItemStatus<'s> {
    pub(crate) fn new(
        item: &'s WhyInScopeAndActionWithItemStatus<'s>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        Self {
            item,
            filter,
            display_format,
        }
    }

    pub(crate) fn get_urgency_now(&self) -> SurrealUrgency {
        self.item.get_urgency_now()
    }

    pub(crate) fn get_action(&self) -> &ActionWithItemStatus<'s> {
        self.item.get_action()
    }

    pub(crate) fn is_in_scope_for_importance(&self) -> bool {
        self.item.is_in_scope_for_importance()
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item.get_surreal_record_id()
    }

    pub(crate) fn get_why_in_scope(&self) -> &HashSet<WhyInScope> {
        self.item.get_why_in_scope()
    }

    pub(crate) fn clone_to_surreal_action(&self) -> SurrealAction {
        self.item.clone_to_surreal_action()
    }
}
