use std::fmt::{Display, Formatter};

use inquire::{InquireError, Select};
use surrealdb::opt::RecordId;
use tokio::sync::mpsc::Sender;

use crate::{
    data_storage::surrealdb_layer::{
        data_layer_commands::{DataLayerCommands, ScopeModeCommand},
        surreal_item::SurrealUrgencyNoData,
        surreal_mode::SurrealScope,
    },
    display::{
        display_item_node::DisplayFormat, display_item_status::DisplayItemStatus,
        display_mode_node::DisplayModeNode,
    },
    node::{item_status::ItemStatus, mode_node::ModeNode, Filter},
};

enum InTheModeChoices {
    Core,
    NonCore,
    OutOfScope,
}

impl Display for InTheModeChoices {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InTheModeChoices::Core => write!(f, "In the mode and a Core reason for the mode"),
            InTheModeChoices::NonCore => write!(
                f,
                "Non-Core for the mode, nevertheless include it in the mode"
            ),
            InTheModeChoices::OutOfScope => write!(f, "Not in the mode, Out of Scope"),
        }
    }
}

pub(crate) async fn present_state_if_in_mode_menu(
    item_status: &ItemStatus<'_>,
    current_mode: &ModeNode<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let selection = Select::new(
        &format!(
            "Is \n{} \nin scope for mode:{}",
            DisplayItemStatus::new(item_status, Filter::Active, DisplayFormat::MultiLineTree),
            DisplayModeNode::new(current_mode, DisplayFormat::SingleLine)
        ),
        vec![
            InTheModeChoices::Core,
            InTheModeChoices::NonCore,
            InTheModeChoices::OutOfScope,
        ],
    )
    .prompt();
    match selection {
        Ok(choice) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::DeclareScopeForMode(match choice {
                    InTheModeChoices::Core => ScopeModeCommand::AddCore {
                        mode: current_mode.get_surreal_id().clone(),
                        scope: prompt_for_surreal_scope(
                            item_status.get_surreal_record_id().clone(),
                        )
                        .await,
                    },
                    InTheModeChoices::NonCore => ScopeModeCommand::AddNonCore {
                        mode: current_mode.get_surreal_id().clone(),
                        scope: prompt_for_surreal_scope(
                            item_status.get_surreal_record_id().clone(),
                        )
                        .await,
                    },
                    InTheModeChoices::OutOfScope => ScopeModeCommand::AddExplicitlyOutOfScope {
                        mode: current_mode.get_surreal_id().clone(),
                        item: item_status.get_surreal_record_id().clone(),
                    },
                }))
                .await
                .unwrap();

            Ok(())
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(InquireError::OperationCanceled) => todo!(),
        Err(_) => todo!(),
    }
}

enum ScopeChoices {
    EverythingInScope,
    OnlySomeUrgenciesInScope,
}

impl Display for ScopeChoices {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeChoices::EverythingInScope => write!(f, "Everything in scope"),
            ScopeChoices::OnlySomeUrgenciesInScope => write!(f, "Only some urgencies in scope"),
        }
    }
}

async fn prompt_for_surreal_scope(item: RecordId) -> SurrealScope {
    let selection = Select::new(
        "Is everything in scope or only some urgencies?",
        vec![
            ScopeChoices::EverythingInScope,
            ScopeChoices::OnlySomeUrgenciesInScope,
        ],
    )
    .prompt()
    .unwrap();

    match selection {
        ScopeChoices::EverythingInScope => SurrealScope {
            for_item: item,
            is_importance_in_scope: true,
            urgencies_to_include: SurrealUrgencyNoData::all(),
        },
        ScopeChoices::OnlySomeUrgenciesInScope => SurrealScope {
            for_item: item,
            is_importance_in_scope: false,
            urgencies_to_include: todo!(),
        },
    }
}
