use std::fmt::Display;

use surrealdb::RecordId;

use crate::{
    base_data::item::Item,
    display::{DisplayStyle, display_item_type::DisplayItemType},
};

/// DisplayItem was created to make it centralize all of the different ways of displaying or printing an Item without
/// putting that onto the core Item type that should not be tied to specific display or printing logic for a console
/// application.
#[derive(Debug)]
pub(crate) struct DisplayItem<'s> {
    pub(crate) item: &'s Item<'s>,
}

impl<'s> From<&&'s Item<'s>> for DisplayItem<'s> {
    fn from(item: &&'s Item<'s>) -> Self {
        DisplayItem { item }
    }
}

impl<'s> From<&'s Item<'s>> for DisplayItem<'s> {
    fn from(item: &'s Item<'s>) -> Self {
        DisplayItem { item }
    }
}

impl<'s> From<DisplayItem<'s>> for &'s Item<'s> {
    fn from(item: DisplayItem<'s>) -> Self {
        item.item
    }
}

impl From<DisplayItem<'_>> for RecordId {
    fn from(display_item: DisplayItem<'_>) -> Self {
        display_item.item.get_surreal_record_id().clone()
    }
}

impl Display for DisplayItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_item_type =
            DisplayItemType::new(DisplayStyle::Abbreviated, self.item.get_item_type());
        write!(f, "{} {}", display_item_type, self.item.get_summary())
    }
}

impl<'s> DisplayItem<'s> {
    pub(crate) fn new(item: &'s Item<'s>) -> Self {
        DisplayItem { item }
    }

    pub(crate) fn get_surreal_record_id(&'s self) -> &'s RecordId {
        self.item.get_surreal_record_id()
    }
}
