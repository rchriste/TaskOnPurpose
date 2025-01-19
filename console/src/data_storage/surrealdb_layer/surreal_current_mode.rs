use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealCurrentMode {
    pub(crate) id: Option<Thing>,
    pub(crate) version: u32,
    pub(crate) mode: Option<Thing>,
}

impl SurrealCurrentMode {
    pub(crate) const TABLE_NAME: &'static str = "current_modes";
}

pub(crate) struct NewCurrentMode {
    current_mode_surreal_id: Option<Thing>,
}

impl From<NewCurrentMode> for SurrealCurrentMode {
    fn from(new_current_mode: NewCurrentMode) -> Self {
        SurrealCurrentMode {
            id: Some((SurrealCurrentMode::TABLE_NAME, "current_mode").into()),
            version: 1,
            mode: new_current_mode.current_mode_surreal_id,
        }
    }
}

impl NewCurrentMode {
    pub(crate) fn new(mode_surreal_id: Option<Thing>) -> Self {
        NewCurrentMode {
            current_mode_surreal_id: mode_surreal_id,
        }
    }
}
