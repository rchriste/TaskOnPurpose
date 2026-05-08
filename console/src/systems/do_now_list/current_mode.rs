use surrealdb::types::RecordId;

use crate::{
    base_data::mode::Mode, data_storage::surrealdb_layer::surreal_current_mode::SurrealCurrentMode,
    node::item_node::ItemNode,
};

#[derive(Clone)]
pub(crate) struct CurrentMode {
    mode_id: Option<RecordId>,
    mode_name: String,
}

impl Default for CurrentMode {
    fn default() -> Self {
        CurrentMode {
            mode_id: None,
            mode_name: "(no mode selected)".to_string(),
        }
    }
}

impl CurrentMode {
    pub(crate) fn new(
        surreal_current_mode: &SurrealCurrentMode,
        modes: &[Mode<'_>],
    ) -> CurrentMode {
        let mode_id = surreal_current_mode.current_mode.clone();
        let mode_name = mode_id
            .as_ref()
            .and_then(|mode_id| {
                modes
                    .iter()
                    .find(|mode| mode.get_surreal_id() == mode_id)
                    .map(|mode| mode.get_name().to_string())
            })
            .unwrap_or_else(|| "(no mode selected)".to_string());

        CurrentMode { mode_id, mode_name }
    }

    pub(crate) fn get_mode_id(&self) -> Option<&RecordId> {
        self.mode_id.as_ref()
    }

    pub(crate) fn get_name(&self) -> &str {
        &self.mode_name
    }

    pub(crate) fn is_urgency_in_the_mode(&self, item_node: &ItemNode) -> bool {
        let _ = item_node;
        true
    }

    pub(crate) fn is_importance_in_the_mode(&self, item_node: &ItemNode) -> bool {
        let _ = item_node;
        true
    }
}
