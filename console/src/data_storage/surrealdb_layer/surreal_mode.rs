use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::new_mode::NewMode;

use super::surreal_item::SurrealUrgencyNoData;

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealMode {
    pub(crate) id: Option<Thing>,
    pub(crate) summary: String,
    pub(crate) version: u32,
    pub(crate) parent_mode: Option<Thing>,
    //What urgencies & importance should be brought in as non-core to this mode but still in scope.
    pub(crate) non_core_in_scope: Vec<SurrealScope>,
    //What urgencies & importance should be brought in as core to this mode and in scope.
    pub(crate) core_in_scope: Vec<SurrealScope>,
    //What urgencies & importance are out of scope meaning that they are not new that needs
    //to be decided but things that have been explicitly decided to not be in scope.
    pub(crate) explicitly_out_of_scope_items: Vec<Thing>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealModeVersion0 {
    pub(crate) id: Option<Thing>,
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) parent: Option<Thing>,
}

impl From<SurrealModeVersion0> for SurrealMode {
    fn from(version0: SurrealModeVersion0) -> Self {
        SurrealMode {
            id: version0.id,
            summary: version0.name,
            version: version0.version,
            parent_mode: version0.parent,
            non_core_in_scope: vec![],
            core_in_scope: vec![],
            explicitly_out_of_scope_items: vec![],
        }
    }
}

impl From<NewMode> for SurrealMode {
    fn from(new_mode: NewMode) -> Self {
        SurrealMode {
            id: None,
            summary: new_mode.summary,
            version: 1,
            parent_mode: new_mode.parent_mode,
            core_in_scope: new_mode.core_in_scope,
            non_core_in_scope: new_mode.non_core_in_scope,
            explicitly_out_of_scope_items: new_mode.explicitly_out_of_scope_items,
        }
    }
}

//For each reason you should be able to state if it is brought in my default or if you need to ask if it should be brought in or not if it can be worked on in that mode
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SurrealScope {
    pub(crate) for_item: Thing,
    pub(crate) is_importance_in_scope: bool,
    pub(crate) urgencies_to_include: Vec<SurrealUrgencyNoData>,
}

impl SurrealMode {
    pub(crate) const TABLE_NAME: &'static str = "modes";
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::data_storage::surrealdb_layer::{
        data_layer_commands::{data_storage_connect_to_db, data_storage_endless_loop},
        surreal_mode::{self, SurrealModeVersion0},
        surreal_tables::SurrealTables,
    };

    #[tokio::test]
    async fn surreal_mode_upgrade_from_version0_to_version1() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle = tokio::spawn(async move {
            // data_storage_start_and_run(receiver, "mem://").await
            let db = data_storage_connect_to_db("mem://").await;

            //Setup the test scenario by saving out a Version0 entry that will need to be upgraded
            let mut version0 = SurrealModeVersion0 {
                id: None,
                name: "Test Mode".to_string(),
                version: 0,
                parent: None,
            };
            let created: SurrealModeVersion0 = db
                .create(surreal_mode::SurrealMode::TABLE_NAME)
                .content(version0.clone())
                .await
                .unwrap()
                .into_iter()
                .next()
                .unwrap();

            version0.id = created.id.clone();
            assert_eq!(version0, created);

            data_storage_endless_loop(db, receiver).await
        });

        let surreal_tables = SurrealTables::new(&sender).await.expect("test failed");

        assert!(!surreal_tables.surreal_modes.is_empty());

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }
}
