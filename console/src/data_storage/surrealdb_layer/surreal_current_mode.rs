use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealCurrentMode {
    pub(crate) id: Option<RecordId>,
    pub(crate) version: u32,
    pub(crate) urgency_in_scope: Vec<SurrealSelectedSingleMode>,
    pub(crate) importance_in_scope: Vec<SurrealSelectedSingleMode>,
}

impl From<SurrealCurrentMode> for Option<RecordId> {
    fn from(value: SurrealCurrentMode) -> Self {
        value.id
    }
}

impl SurrealCurrentMode {
    pub(crate) const TABLE_NAME: &'static str = "current_modes";
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealSelectedSingleMode {
    AllCoreMotivationalPurposes,
    AllNonCoreMotivationalPurposes,
}

pub(crate) struct NewCurrentMode {
    urgency_in_scope: Vec<SurrealSelectedSingleMode>,
    importance_in_scope: Vec<SurrealSelectedSingleMode>,
}

impl From<NewCurrentMode> for SurrealCurrentMode {
    fn from(new_current_mode: NewCurrentMode) -> Self {
        SurrealCurrentMode {
            id: Some((SurrealCurrentMode::TABLE_NAME, "current_mode").into()),
            version: 0,
            urgency_in_scope: new_current_mode.urgency_in_scope,
            importance_in_scope: new_current_mode.importance_in_scope,
        }
    }
}

impl NewCurrentMode {
    pub(crate) fn new(
        urgency_in_scope: Vec<SurrealSelectedSingleMode>,
        importance_in_scope: Vec<SurrealSelectedSingleMode>,
    ) -> Self {
        NewCurrentMode {
            urgency_in_scope,
            importance_in_scope,
        }
    }
}
