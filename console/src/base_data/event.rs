use chrono::{DateTime, Utc};
use surrealdb::RecordId;

use crate::data_storage::surrealdb_layer::surreal_event::SurrealEvent;

#[derive(Debug, Eq)]
pub(crate) struct Event<'s> {
    id: &'s RecordId,
    last_updated: DateTime<Utc>,
    surreal_event: &'s SurrealEvent,
}

impl PartialEq for Event<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<'s> Event<'s> {
    pub(crate) fn new(surreal_event: &'s SurrealEvent) -> Self {
        let id = surreal_event.id.as_ref().expect("In DB");
        let last_updated = surreal_event.last_updated.clone().into();
        Event {
            id,
            surreal_event,
            last_updated,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.surreal_event.triggered
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.id
    }

    pub(crate) fn get_summary(&self) -> &str {
        &self.surreal_event.summary
    }

    pub(crate) fn get_last_updated(&self) -> &DateTime<Utc> {
        &self.last_updated
    }
}
