use std::fmt::{Display, Formatter};

use crate::{
    display::{display_item_status::DisplayItemStatus, display_mode::DisplayMode},
    node::{Filter, action_with_item_status::ActionWithItemStatus},
};

use super::display_item_node::DisplayFormat;

#[derive(Clone)]
pub(crate) struct DisplayActionWithItemStatus<'s> {
    item: &'s ActionWithItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayActionWithItemStatus<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.item {
            ActionWithItemStatus::MakeProgress(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(f, "[🏃 Do Now] {}", display)
            }
            ActionWithItemStatus::ParentBackToAMotivation(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(f, "[🌟 Needs a reason] {}", display)
            }
            ActionWithItemStatus::PickItemReviewFrequency(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(f, "[🔁 State review frequency] {}", display)
            }
            ActionWithItemStatus::ItemNeedsAClassification(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(f, "[🗂️ Needs classification] {}", display)
            }
            ActionWithItemStatus::ReviewItem(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                if let Some(review_frequency) =
                    item_status.get_item().get_surreal_review_frequency()
                {
                    write!(f, "[🔍 Review - {}] {}", review_frequency, display)
                } else {
                    write!(f, "[🔍 Review] {}", display)
                }
            }
            ActionWithItemStatus::SetReadyAndUrgency(item_status) => {
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(f, "[🚦 Set readiness and urgency] {}", display)
            }
            ActionWithItemStatus::StateIfInMode(item_status, current_mode) => {
                let display_mode = DisplayMode::new(current_mode.get_mode());
                let display = DisplayItemStatus::new(item_status, self.filter, self.display_format);
                write!(
                    f,
                    "[🔒 Is in scope] Mode: {} Item: {}",
                    display_mode, display
                )
            }
        }
    }
}

impl<'s> DisplayActionWithItemStatus<'s> {
    pub(crate) fn new(
        item: &'s ActionWithItemStatus<'s>,
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
