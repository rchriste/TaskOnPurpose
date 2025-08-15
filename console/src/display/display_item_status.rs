use std::fmt::Display;

use crate::node::{Filter, item_status::ItemStatus};

use super::display_item_node::{DisplayFormat, DisplayItemNode};

pub struct DisplayItemStatus<'s> {
    item_status: &'s ItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayItemStatus<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "|")?;
        if self.has_dependencies(self.filter) {
            write!(f, "⏳ ")?;
        }

        let display_node = DisplayItemNode::new(
            self.item_status.get_item_node(),
            self.filter,
            self.display_format,
        );
        write!(f, "{}", display_node)?;
        Ok(())
    }
}

impl<'s> DisplayItemStatus<'s> {
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

    pub(crate) fn get_item_status(&self) -> &'s ItemStatus<'s> {
        self.item_status
    }

    pub(crate) fn has_children(&self, filter: Filter) -> bool {
        self.item_status.has_children(filter)
    }

    pub(crate) fn has_dependencies(&self, filter: Filter) -> bool {
        self.item_status.has_dependencies(filter)
    }
}
