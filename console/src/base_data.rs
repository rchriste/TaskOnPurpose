pub(crate) mod event;
pub(crate) mod in_the_moment_priority;
pub(crate) mod item;
pub(crate) mod mode;
pub(crate) mod time_spent;

use ahash::HashMap;
use chrono::{DateTime, Utc};
use ouroboros::self_referencing;
use surrealdb::RecordId;

use crate::data_storage::surrealdb_layer::{
    surreal_current_mode::SurrealCurrentMode,
    surreal_in_the_moment_priority::SurrealInTheMomentPriority, surreal_tables::SurrealTables,
};

use self::{
    event::Event,
    item::{Item, ItemVecExtensions},
    mode::Mode,
    time_spent::TimeSpent,
};

#[self_referencing]
pub(crate) struct BaseData {
    surreal_tables: SurrealTables,
    now: DateTime<Utc>,

    #[borrows(surreal_tables, now)]
    #[covariant]
    items: HashMap<&'this RecordId, Item<'this>>,

    #[borrows(items)]
    #[covariant]
    active_items: Vec<&'this Item<'this>>,

    #[borrows(surreal_tables)]
    #[covariant]
    events: HashMap<&'this RecordId, Event<'this>>,

    #[borrows(surreal_tables)]
    #[covariant]
    time_spent_log: Vec<TimeSpent<'this>>,

    #[borrows(surreal_tables)]
    #[covariant]
    modes: Vec<Mode<'this>>,
}

impl BaseData {
    pub(crate) fn new_from_surreal_tables(
        surreal_tables: SurrealTables,
        now: DateTime<Utc>,
    ) -> Self {
        BaseDataBuilder {
            surreal_tables,
            items_builder: |surreal_tables, now| surreal_tables.make_items(now),
            active_items_builder: |items| items.filter_active_items(),
            events_builder: |surreal_tables| surreal_tables.make_events(),
            now,
            time_spent_log_builder: |surreal_tables| surreal_tables.make_time_spent_log().collect(),
            modes_builder: |surreal_tables| surreal_tables.make_modes().collect(),
        }
        .build()
    }

    pub(crate) fn get_now(&self) -> &DateTime<Utc> {
        self.borrow_now()
    }

    pub(crate) fn get_items(&self) -> &HashMap<&RecordId, Item<'_>> {
        self.borrow_items()
    }

    pub(crate) fn get_active_items(&self) -> &[&Item<'_>] {
        self.borrow_active_items()
    }

    pub(crate) fn get_events(&self) -> &HashMap<&RecordId, Event<'_>> {
        self.borrow_events()
    }

    pub(crate) fn get_time_spent_log(&self) -> &[TimeSpent<'_>] {
        self.borrow_time_spent_log()
    }

    pub(crate) fn get_surreal_in_the_moment_priorities(&self) -> &[SurrealInTheMomentPriority] {
        self.borrow_surreal_tables()
            .get_surreal_in_the_moment_priorities()
    }

    pub(crate) fn get_surreal_current_modes(&self) -> &[SurrealCurrentMode] {
        self.borrow_surreal_tables().get_surreal_current_modes()
    }

    pub(crate) fn get_modes(&self) -> &[Mode<'_>] {
        self.borrow_modes()
    }
}

pub(crate) struct Visited<'s, 'v> {
    myself: &'s RecordId,
    parent: Option<&'v Visited<'s, 'v>>,
}

impl<'s, 'v> Visited<'s, 'v> {
    pub(crate) fn new(new: &'s RecordId, existing: Option<&'v Visited<'s, 'v>>) -> Self {
        Visited {
            myself: new,
            parent: existing,
        }
    }

    pub(crate) fn contains(&self, record_id: &RecordId) -> bool {
        if self.myself == record_id {
            true
        } else {
            match self.parent {
                Some(parent) => parent.contains(record_id),
                None => false,
            }
        }
    }
}
