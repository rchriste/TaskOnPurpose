use std::fmt::Display;

use crate::{
    display::{
        DisplayStyle, display_item_node::DisplayFormat, display_item_status::DisplayItemStatus,
        display_urgency_plan::DisplayUrgency,
    },
    node::{Filter, item_status::ItemStatus},
};

pub struct DisplayItemStatusWithUrgency<'s> {
    item_status: &'s ItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayItemStatusWithUrgency<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(urgency) = self.item_status.get_urgency_now() {
            let display_urgency = DisplayUrgency::new(urgency, DisplayStyle::Abbreviated);
            write!(f, "{} ", display_urgency)?;
        }

        let display_status =
            DisplayItemStatus::new(self.item_status, self.filter, self.display_format);
        write!(f, "{}", display_status)
    }
}

impl<'s> DisplayItemStatusWithUrgency<'s> {
    pub(crate) fn new(
        item_status: &'s ItemStatus,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        Self {
            item_status,
            filter,
            display_format,
        }
    }
}
