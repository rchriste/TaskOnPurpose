use serde::{Deserialize, Serialize};
use surrealdb::{RecordId, sql::Datetime};

use crate::new_time_spent::NewTimeSpent;

use super::{surreal_in_the_moment_priority::SurrealAction, surreal_item::SurrealUrgency};

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealTimeSpent {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) working_on: Vec<SurrealAction>,
    pub(crate) why_in_scope: Vec<SurrealWhyInScope>,
    pub(crate) urgency: Option<SurrealUrgency>,
    pub(crate) when_started: Datetime,
    pub(crate) when_stopped: Datetime,
    pub(crate) dedication: Option<SurrealDedication>,
}

impl From<SurrealTimeSpent> for Option<RecordId> {
    fn from(value: SurrealTimeSpent) -> Self {
        value.id
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Hash)]
pub(crate) enum SurrealWhyInScope {
    Importance,
    Urgency,
    MenuNavigation,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealDedication {
    PrimaryTask,
    BackgroundTask,
}

impl From<NewTimeSpent> for SurrealTimeSpent {
    fn from(new_time_spent: NewTimeSpent) -> Self {
        SurrealTimeSpent {
            id: None,
            version: 1,
            why_in_scope: new_time_spent.why_in_scope,
            working_on: new_time_spent.working_on,
            when_started: new_time_spent.when_started.into(),
            when_stopped: new_time_spent.when_stopped.into(),
            dedication: new_time_spent.dedication,
            urgency: new_time_spent.urgency,
        }
    }
}

impl SurrealTimeSpent {
    pub(crate) const TABLE_NAME: &'static str = "time_spent_log";
}

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealTimeSpentVersion0 {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) working_on: Vec<SurrealAction>,
    pub(crate) urgency: Option<SurrealUrgency>,
    pub(crate) when_started: Datetime,
    pub(crate) when_stopped: Datetime,
    pub(crate) dedication: Option<SurrealDedication>,
}

impl From<SurrealTimeSpentVersion0> for SurrealTimeSpent {
    fn from(old: SurrealTimeSpentVersion0) -> Self {
        //This is a best effort conversion. In the scenario where you have something that is both urgent and important
        //it will only be marked down as urgent because the information doesn't exist to know that it is both and is
        //also important. Otherwise the conversation should be correct.
        let why_in_scope = match old.urgency {
            Some(SurrealUrgency::InTheModeByImportance) => vec![SurrealWhyInScope::Importance],
            Some(SurrealUrgency::InTheModeDefinitelyUrgent)
            | Some(SurrealUrgency::InTheModeMaybeUrgent)
            | Some(SurrealUrgency::InTheModeScheduled(..))
            | Some(SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled)
            | Some(SurrealUrgency::MoreUrgentThanMode)
            | Some(SurrealUrgency::ScheduledAnyMode(..)) => vec![SurrealWhyInScope::Urgency],
            None => vec![],
        };
        SurrealTimeSpent {
            id: old.id,
            version: 1,
            working_on: old.working_on,
            why_in_scope,
            urgency: old.urgency,
            when_started: old.when_started,
            when_stopped: old.when_stopped,
            dedication: old.dedication,
        }
    }
}
