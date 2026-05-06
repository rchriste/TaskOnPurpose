use inquire::InquireError;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, mode::Mode},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_current_mode::NewCurrentMode,
        surreal_tables::SurrealTables,
    },
    display::display_mode_node::DisplayModeNode,
    menu::inquire::{
        default_select_page_size,
        do_now_list_menu::{ShouldResumeCurrentlyWorkingOn, present_normal_do_now_list_menu},
    },
    node::mode_node::ModeNode,
    systems::do_now_list::current_mode::CurrentMode,
};

enum ChangeModeOption<'e> {
    Clear,
    Mode(&'e ModeNode<'e>),
}

impl std::fmt::Display for ChangeModeOption<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clear => write!(f, "Clear current mode"),
            Self::Mode(mode) => write!(
                f,
                "{}",
                DisplayModeNode::new(
                    mode,
                    crate::display::display_item_node::DisplayFormat::SingleLine
                )
            ),
        }
    }
}

pub(crate) async fn present_change_mode_menu(
    current_mode: &CurrentMode,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();

    let now = chrono::Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let calculated_data = CalculatedData::new_from_base_data(base_data);

    let mut mode_nodes = calculated_data.get_mode_nodes().iter().collect::<Vec<_>>();
    mode_nodes.sort_by(|a, b| {
        use std::cmp::Ordering;

        fn compare_chains(
            mut a_parent_chain: Vec<&Mode<'_>>,
            mut b_parent_chain: Vec<&Mode<'_>>,
        ) -> Ordering {
            let a_parent_chain_last = a_parent_chain.last();
            let b_parent_chain_last = b_parent_chain.last();
            match (a_parent_chain_last, b_parent_chain_last) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(a_parent_chain_last), Some(b_parent_chain_last)) => {
                    let ordering = a_parent_chain_last
                        .get_name()
                        .cmp(b_parent_chain_last.get_name());
                    if let Ordering::Equal = ordering {
                        a_parent_chain.pop();
                        b_parent_chain.pop();
                        compare_chains(a_parent_chain, b_parent_chain)
                    } else {
                        ordering
                    }
                }
            }
        }

        let a_parent_chain = a.create_parent_chain();
        let b_parent_chain = b.create_parent_chain();
        compare_chains(a_parent_chain, b_parent_chain)
    });

    if mode_nodes.is_empty() {
        println!("No modes exist yet. Create one first in Back Menu -> Configure Modes.");
        return Ok(());
    }

    let mut options = Vec::with_capacity(mode_nodes.len() + 1);
    options.push(ChangeModeOption::Clear);
    options.extend(mode_nodes.into_iter().map(ChangeModeOption::Mode));

    let default = current_mode
        .get_mode_id()
        .and_then(|mode_id| {
            options.iter().position(|opt| match opt {
                ChangeModeOption::Clear => false,
                ChangeModeOption::Mode(mode_node) => mode_node.get_surreal_id() == mode_id,
            })
        })
        .unwrap_or(0);

    let selection = inquire::Select::new("Select the current mode", options)
        .with_page_size(default_select_page_size())
        .with_starting_cursor(default)
        .prompt();

    let mode_id = match selection {
        Ok(ChangeModeOption::Clear) => None,
        Ok(ChangeModeOption::Mode(mode)) => Some(mode.get_surreal_id().clone()),
        Err(InquireError::OperationCanceled) => {
            return Box::pin(present_normal_do_now_list_menu(
                send_to_data_storage_layer,
                ShouldResumeCurrentlyWorkingOn::AlwaysLoadDoNowList,
            ))
            .await;
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    let new_current_mode = NewCurrentMode::new(mode_id);
    send_to_data_storage_layer
        .send(DataLayerCommands::SetCurrentMode(new_current_mode))
        .await
        .unwrap();

    Ok(())
}
