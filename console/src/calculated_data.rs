pub(crate) mod parent_lookup;

use crate::{
    base_data::{
        event::Event, in_the_moment_priority::InTheMomentPriorityWithItemAction, item::Item,
        time_spent::TimeSpent, BaseData,
    },
    node::{item_node::ItemNode, item_status::ItemStatus, mode_node::ModeNode},
    systems::do_now_list::current_mode::CurrentMode,
};
use ahash::HashMap;
use chrono::{DateTime, Utc};
use ouroboros::self_referencing;
use surrealdb::opt::RecordId;

use parent_lookup::ParentLookup;

#[self_referencing]
pub(crate) struct CalculatedData {
    base_data: BaseData,

    #[borrows(base_data)]
    #[covariant]
    parent_lookup: ParentLookup<'this>,

    #[borrows(base_data, parent_lookup)]
    #[covariant]
    items_nodes: HashMap<&'this RecordId, ItemNode<'this>>,

    #[borrows(items_nodes)]
    #[covariant]
    items_status: HashMap<&'this RecordId, ItemStatus<'this>>,

    #[borrows(base_data)]
    #[covariant]
    mode_nodes: Vec<ModeNode<'this>>,

    #[borrows(items_status, base_data, items_nodes, mode_nodes)]
    #[covariant]
    in_the_moment_priorities: Vec<InTheMomentPriorityWithItemAction<'this>>,

    #[borrows(base_data, mode_nodes)]
    #[covariant]
    current_mode: Option<CurrentMode<'this>>,
}

impl CalculatedData {
    pub(crate) fn new_from_base_data(base_data: BaseData) -> Self {
        CalculatedDataBuilder {
            base_data,
            parent_lookup_builder: |base_data| {
                ParentLookup::new(base_data.get_items())
            },
            items_nodes_builder: |base_data, parent_lookup| {
                base_data
                    .get_items()
                    .iter()
                    .map(|(k, x)| {
                        (
                            *k,
                            ItemNode::new(x, base_data.get_items(), parent_lookup, base_data.get_events(), base_data.get_time_spent_log()),
                        )
                    })
                    .collect::<HashMap<_, _>>()
            },
            items_status_builder: |item_nodes| {
                item_nodes
                    .iter()
                    .map(|(k, x)| (*k, ItemStatus::new(x, item_nodes)))
                    .collect::<HashMap<_, _>>()
            },
            in_the_moment_priorities_builder: |items_status, base_data, all_nodes, all_modes| {
                let now_sql = (*base_data.get_now()).into();
                let all_items = base_data.get_items();
                let time_spent_log = base_data.get_time_spent_log();
                let mut in_the_moment_priorities = base_data
                    .get_surreal_in_the_moment_priorities()
                    .iter()
                    .map(|x| {
                        InTheMomentPriorityWithItemAction::new(
                            x,
                            &now_sql,
                            all_items,
                            all_nodes,
                            items_status,
                            all_modes,
                            time_spent_log,
                        )
                    })
                    .collect::<Vec<_>>();
                //We are sorting by created so the order is consistent. This is meant to make it so when priorities are applied making a new priority won't cover up or erase an older already set priority.
                in_the_moment_priorities.sort_by(|a, b| {
                    a.get_created().cmp(b.get_created())
                });
                in_the_moment_priorities
            },
            mode_nodes_builder: |base_data| {
                let all_modes = base_data.get_modes();
                all_modes.iter().map(|x| ModeNode::new(x, all_modes)).collect()
            },
            current_mode_builder: |base_data, mode_nodes| {
                let surreal_current_modes = base_data.get_surreal_current_modes();
                assert!(surreal_current_modes.len() <= 1, "There should not be more than one current mode. In the future it would be great for this to be a silent error that just deletes the current mode and logs an error into an error log (that currently doesn't exist) clears it but for now this is an assert. Length is: {}", surreal_current_modes.len());
                let surreal_current_mode = surreal_current_modes.first();
                surreal_current_mode.map(|surreal_current_mode| {
                        CurrentMode::new(surreal_current_mode, mode_nodes)
                })
            },
        }
        .build()
    }

    pub(crate) fn get_items(&self) -> &HashMap<&RecordId, Item> {
        self.borrow_base_data().get_items()
    }

    pub(crate) fn get_active_items(&self) -> &[&Item] {
        self.borrow_base_data().get_active_items()
    }

    pub(crate) fn get_items_status(&self) -> &HashMap<&RecordId, ItemStatus<'_>> {
        self.borrow_items_status()
    }

    pub(crate) fn get_in_the_moment_priorities(&self) -> &[InTheMomentPriorityWithItemAction<'_>] {
        self.borrow_in_the_moment_priorities()
    }

    pub(crate) fn get_now(&self) -> &DateTime<Utc> {
        self.borrow_base_data().get_now()
    }

    pub(crate) fn get_time_spent_log(&self) -> &[TimeSpent<'_>] {
        self.borrow_base_data().get_time_spent_log()
    }

    pub(crate) fn get_current_mode(&self) -> &Option<CurrentMode> {
        self.borrow_current_mode()
    }

    pub(crate) fn get_mode_nodes(&self) -> &[ModeNode<'_>] {
        self.borrow_mode_nodes()
    }

    pub(crate) fn get_events(&self) -> &HashMap<&RecordId, Event<'_>> {
        self.borrow_base_data().get_events()
    }

    pub(crate) fn get_base_data(&self) -> &BaseData {
        self.borrow_base_data()
    }
}
