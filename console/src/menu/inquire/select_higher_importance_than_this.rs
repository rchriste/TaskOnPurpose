use std::fmt::Display;

use inquire::Select;
use surrealdb::RecordId;

use crate::{
    base_data::item::Item, display::display_item::DisplayItem,
    menu::inquire::default_select_page_size,
};

#[derive(Debug)]
pub(crate) enum HigherImportanceThan<'e> {
    Item(DisplayItem<'e>),
    PutAtTheBottom,
}

impl Display for HigherImportanceThan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HigherImportanceThan::Item(display_item) => write!(f, "{}", display_item),
            HigherImportanceThan::PutAtTheBottom => write!(f, "Put at the bottom"),
        }
    }
}

impl From<HigherImportanceThan<'_>> for Option<RecordId> {
    fn from(higher_priority_than: HigherImportanceThan<'_>) -> Self {
        match higher_priority_than {
            HigherImportanceThan::Item(display_item) => Some(display_item.into()),
            HigherImportanceThan::PutAtTheBottom => None,
        }
    }
}

impl<'e> HigherImportanceThan<'e> {
    pub(crate) fn create_list(items: &'e [&'e Item<'_>]) -> Vec<HigherImportanceThan<'e>> {
        let mut list = Vec::with_capacity(items.len() + 1);
        for item in items.iter() {
            let display_item = DisplayItem::new(item);
            list.push(HigherImportanceThan::Item(display_item));
        }
        list.push(HigherImportanceThan::PutAtTheBottom);
        list
    }
}

#[must_use]
pub(crate) fn select_higher_importance_than_this(
    items: &[&Item<'_>],
    starting_position: Option<usize>,
) -> Option<RecordId> {
    let list = HigherImportanceThan::create_list(items);
    let starting_position = starting_position.unwrap_or(0);
    println!();
    let selected = Select::new(
        "What is the Importance? This is more important than the item you select.|",
        list,
    )
    .with_starting_cursor(starting_position)
    .with_page_size(default_select_page_size())
    .prompt()
    .unwrap();
    match selected {
        HigherImportanceThan::Item(display_item) => {
            let surreal_item = display_item.get_surreal_record_id();
            Some(surreal_item.clone())
        }
        HigherImportanceThan::PutAtTheBottom => None,
    }
}
