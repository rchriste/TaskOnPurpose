use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

use crate::new_mode::NewMode;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealMode {
    pub(crate) id: Option<RecordId>,
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) parent: Option<RecordId>,
}

impl From<NewMode> for SurrealMode {
    fn from(new_mode: NewMode) -> Self {
        SurrealMode {
            id: None,
            name: new_mode.name,
            version: 0,
            parent: new_mode.parent,
        }
    }
}

impl SurrealMode {
    pub(crate) const TABLE_NAME: &'static str = "modes";
}
