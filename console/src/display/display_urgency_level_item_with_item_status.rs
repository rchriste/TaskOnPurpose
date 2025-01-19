use std::fmt::{Display, Formatter};

use crate::{
    data_storage::surrealdb_layer::surreal_item::SurrealUrgency,
    display::display_why_in_scope_and_action_with_item_status::DisplayWhyInScopeAndActionWithItemStatus,
    node::{Filter, urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus},
};

use super::display_item_node::DisplayFormat;

#[derive(Clone)]
pub(crate) struct DisplayUrgencyLevelItemWithItemStatus<'s> {
    item: &'s UrgencyLevelItemWithItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayUrgencyLevelItemWithItemStatus<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.item {
            UrgencyLevelItemWithItemStatus::SingleItem(item) => {
                let display = DisplayWhyInScopeAndActionWithItemStatus::new(
                    item,
                    self.filter,
                    self.display_format,
                );
                write!(f, "{}", display)
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(items) => {
                //I'm drawing this in what looks like reverse order because the point is that if you select anything in the list then the lowest option is something that might be picked and this also makes the list when viewed from the do now list show the main priority first. And because anything can hold the top item that is shown last
                if items
                    .iter()
                    .any(|x| matches!(x.get_urgency_now(), Some(SurrealUrgency::MaybeUrgent(_))))
                {
                    write!(f, "🟡")?;
                }

                if items
                    .iter()
                    .any(|x| matches!(x.get_urgency_now(), Some(SurrealUrgency::Scheduled(..))))
                {
                    write!(f, "🗓️")?;
                }

                if items.iter().any(|x| {
                    matches!(
                        x.get_urgency_now(),
                        Some(SurrealUrgency::DefinitelyUrgent(_))
                    )
                }) {
                    write!(f, "🔴")?;
                }

                if items
                    .iter()
                    .any(|x| matches!(x.get_urgency_now(), Some(SurrealUrgency::CrisesUrgent(_))))
                {
                    write!(f, "🔥")?;
                }

                if items.iter().any(|x| x.is_in_scope_for_importance()) {
                    write!(f, "🔝")?;
                }

                write!(f, " [🗳️  Pick highest priority] {} choices", items.len())
            }
        }
    }
}

impl<'s> DisplayUrgencyLevelItemWithItemStatus<'s> {
    pub(crate) fn new(
        item: &'s UrgencyLevelItemWithItemStatus<'s>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        Self {
            item,
            filter,
            display_format,
        }
    }
}
