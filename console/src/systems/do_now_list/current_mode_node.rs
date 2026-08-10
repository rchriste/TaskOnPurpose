use itertools::Itertools;
use surrealdb::RecordId;

use crate::{node::mode_node::ModeNode, systems::do_now_list::current_mode::CurrentMode};

#[derive(Debug)]
pub(crate) struct CurrentModeNode<'s> {
    mode_node: Option<&'s ModeNode<'s>>,
    current_mode: &'s CurrentMode,
}

impl<'s> CurrentModeNode<'s> {
    pub(crate) fn new(
        current_mode: &'s CurrentMode,
        modes: &'s [ModeNode<'s>],
    ) -> CurrentModeNode<'s> {
        if let Some(current_record_id) = current_mode.get_mode_id() {
            CurrentModeNode {
                mode_node: modes
                    .iter()
                    .find_or_first(|x| x.get_surreal_id() == current_record_id),
                current_mode,
            }
        } else {
            CurrentModeNode {
                mode_node: None,
                current_mode,
            }
        }
    }

    pub(crate) fn get_mode_id(&self) -> Option<&RecordId> {
        self.current_mode.get_mode_id()
    }

    pub(crate) fn get_mode_node(&self) -> Option<&ModeNode<'s>> {
        self.mode_node
    }
}
