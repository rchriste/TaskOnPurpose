use surrealdb::RecordId;

use crate::data_storage::surrealdb_layer::surreal_mode::SurrealMode;

pub(crate) struct Mode<'s> {
    surreal_mode: &'s SurrealMode,
}

impl<'s> Mode<'s> {
    pub(crate) fn new(surreal_mode: &'s SurrealMode) -> Self {
        Self { surreal_mode }
    }

    pub(crate) fn get_name(&self) -> &'s str {
        &self.surreal_mode.name
    }

    pub(crate) fn get_parent(&self) -> &'s Option<RecordId> {
        &self.surreal_mode.parent
    }

    pub(crate) fn get_surreal_id(&self) -> &'s RecordId {
        self.surreal_mode
            .id
            .as_ref()
            .expect("Comes from the database so this is always present")
    }
}
