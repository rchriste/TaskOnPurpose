use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealCurrentMode {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) current_mode: Option<RecordId>,
}

impl From<SurrealCurrentMode> for Option<RecordId> {
    fn from(value: SurrealCurrentMode) -> Self {
        value.id
    }
}

impl SurrealCurrentMode {
    pub(crate) const TABLE_NAME: &'static str = "current_modes";
}
pub(crate) struct NewCurrentMode {
    current_mode: Option<RecordId>,
}

impl From<NewCurrentMode> for SurrealCurrentMode {
    fn from(new_current_mode: NewCurrentMode) -> Self {
        SurrealCurrentMode {
            id: Some((SurrealCurrentMode::TABLE_NAME, "current_mode").into()),
            version: 0,
            current_mode: new_current_mode.current_mode,
        }
    }
}

impl NewCurrentMode {
    pub(crate) fn new(current_mode: Option<RecordId>) -> Self {
        NewCurrentMode { current_mode }
    }
}
