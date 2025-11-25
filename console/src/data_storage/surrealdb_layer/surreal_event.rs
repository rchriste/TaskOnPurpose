use serde::{Deserialize, Serialize};
use surrealdb::{RecordId, sql::Datetime};

use crate::new_event::NewEvent;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealEvent {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) last_updated: Datetime,
    pub(crate) triggered: bool,
    pub(crate) summary: String,
}

impl From<NewEvent> for SurrealEvent {
    fn from(new_event: NewEvent) -> Self {
        SurrealEvent {
            id: None,
            version: 0,
            last_updated: new_event.last_updated.into(),
            triggered: new_event.triggered,
            summary: new_event.summary,
        }
    }
}

impl SurrealEvent {
    pub(crate) const TABLE_NAME: &'static str = "events";
}
