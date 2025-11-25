use serde::{Deserialize, Serialize};
use surrealdb::{
    RecordId,
    sql::{Datetime, Duration},
};

pub(crate) mod data_layer_commands;
pub(crate) mod surreal_current_mode;
pub(crate) mod surreal_event;
pub(crate) mod surreal_in_the_moment_priority;
pub(crate) mod surreal_item;
pub(crate) mod surreal_mode;
pub(crate) mod surreal_tables;
pub(crate) mod surreal_time_spent;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealTrigger {
    WallClockDateTime(Datetime),
    LoggedInvocationCount {
        starting: Datetime,
        count: u32,
        items_in_scope: SurrealItemsInScope,
    },
    LoggedAmountOfTime {
        starting: Datetime,
        duration: Duration,
        items_in_scope: SurrealItemsInScope,
    },
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealItemsInScope {
    All,
    Include(Vec<RecordId>),
    Exclude(Vec<RecordId>),
}
