use std::fmt::Display;

use inquire::Select;
use surrealdb::opt::RecordId;

use crate::{
    base_data::item::Item, data_storage::surrealdb_layer::surreal_item::SurrealModeScope,
    display::display_item::DisplayItem, menu::inquire::prompt_for_mode_scope,
    node::mode_node::ModeNode,
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
    all_modes: &[ModeNode<'_>],
    starting_position: Option<usize>,
) -> Option<(SurrealModeScope, Option<RecordId>)> {
    let list = HigherImportanceThan::create_list(items);
    let starting_position = starting_position.unwrap_or(0);
    println!();
    let selected = Select::new("Select higher importance than this|", list)
        .with_starting_cursor(starting_position)
        .prompt()
        .unwrap();
    let higher_than_this = match selected {
        HigherImportanceThan::Item(display_item) => {
            let surreal_item = display_item.get_surreal_record_id();
            Some(surreal_item.clone())
        }
        HigherImportanceThan::PutAtTheBottom => None,
    };
    let mode_scope = prompt_for_mode_scope(all_modes, todo!());
    Some((mode_scope, higher_than_this))
}
