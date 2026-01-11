use serde::{Deserialize, Serialize};
use surrealdb::{RecordId, sql::Datetime};

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealWorkingOn {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) item: RecordId,
    pub(crate) when_started: Datetime,
}

impl From<SurrealWorkingOn> for Option<RecordId> {
    fn from(value: SurrealWorkingOn) -> Self {
        value.id
    }
}

impl SurrealWorkingOn {
    pub(crate) const TABLE_NAME: &'static str = "working_ons";

    pub(crate) fn new(item: RecordId, when_started: Datetime) -> Self {
        SurrealWorkingOn {
            id: Some((Self::TABLE_NAME, "working_on").into()),
            version: 0,
            item,
            when_started,
        }
    }
}
