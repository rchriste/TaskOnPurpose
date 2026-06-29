use ahash::{HashMap, HashMapExt};
use surrealdb::RecordId;

use crate::base_data::item::Item;

pub(crate) struct ParentLookup<'s> {
    pub(crate) parent_lookup: HashMap<&'s RecordId, Vec<&'s Item<'s>>>,
}

impl<'s> ParentLookup<'s> {
    pub(crate) fn new(all_items: &'s HashMap<&'s RecordId, Item<'s>>) -> Self {
        let mut parent_lookup = HashMap::new();
        for (_, parent_item) in all_items.iter() {
            for child in parent_item.get_children() {
                parent_lookup
                    .entry(child)
                    .or_insert_with(Vec::new)
                    .push(parent_item);
            }
        }
        ParentLookup { parent_lookup }
    }
}
