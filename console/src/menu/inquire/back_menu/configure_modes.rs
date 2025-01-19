use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
};

use chrono::Utc;
use inquire::{InquireError, Select};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, mode::Mode},
    calculated_data::CalculatedData,
    display::display_mode_node::DisplayModeNode,
    menu::inquire::back_menu::{SurrealTables, present_back_menu},
    new_mode::NewModeBuilder,
    node::mode_node::ModeNode,
};

use super::{DataLayerCommands, DisplayFormat};

enum ConfigureModesOptions<'e> {
    Add,
    Done,
    Mode(&'e ModeNode<'e>),
}

impl Display for ConfigureModesOptions<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigureModesOptions::Add => write!(f, "Add New Top Level Mode"),
            ConfigureModesOptions::Done => write!(f, "Done (Return to \"Do Now\" List)"),
            ConfigureModesOptions::Mode(mode) => write!(
                f,
                "{}",
                DisplayModeNode::new(mode, DisplayFormat::SingleLine)
            ),
        }
    }
}

enum ConfigureModesOptionsSelected<'e> {
    AddWithParent(&'e ModeNode<'e>),
    EditName(&'e ModeNode<'e>),
    Back,
    Done,
}

impl Display for ConfigureModesOptionsSelected<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigureModesOptionsSelected::AddWithParent(parent) => write!(
                f,
                "Add New Mode Under {}",
                DisplayModeNode::new(parent, DisplayFormat::SingleLine)
            ),
            ConfigureModesOptionsSelected::EditName(mode) => write!(
                f,
                "Edit Name of {}",
                DisplayModeNode::new(mode, DisplayFormat::SingleLine)
            ),
            ConfigureModesOptionsSelected::Back => write!(f, "Back"),
            ConfigureModesOptionsSelected::Done => write!(f, "Done (Return to \"Do Now\" List)"),
        }
    }
}

pub(crate) async fn configure_modes(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();

    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let calculated_data = CalculatedData::new_from_base_data(base_data);

    let mut options = vec![ConfigureModesOptions::Add];
    let mut mode_nodes = calculated_data.get_mode_nodes().iter().collect::<Vec<_>>();
    mode_nodes.sort_by(|a, b| {
        fn compare_chains(
            mut a_self_parent_chain: Vec<&Mode<'_>>,
            mut b_self_parent_chain: Vec<&Mode<'_>>,
        ) -> Ordering {
            let a_parent_chain_last = a_self_parent_chain.last();
            let b_parent_chain_last = b_self_parent_chain.last();
            match (a_parent_chain_last, b_parent_chain_last) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(a_parent_chain_last), Some(b_parent_chain_last)) => {
                    let ordering = a_parent_chain_last
                        .get_name()
                        .cmp(b_parent_chain_last.get_name());
                    if let Ordering::Equal = ordering {
                        a_self_parent_chain.pop();
                        b_self_parent_chain.pop();
                        compare_chains(a_self_parent_chain, b_self_parent_chain)
                    } else {
                        ordering
                    }
                }
            }
        }

        let a_self_parent_chain = a.create_self_parent_chain();
        let b_self_parent_chain = b.create_self_parent_chain();
        compare_chains(a_self_parent_chain, b_self_parent_chain)
    });

    options.extend(mode_nodes.into_iter().map(ConfigureModesOptions::Mode));
    options.push(ConfigureModesOptions::Done);

    println!();

    let selection = Select::new("Select a mode", options).prompt();
    match selection {
        Ok(ConfigureModesOptions::Add) => {
            let name = inquire::Text::new("Enter the name of the new mode").prompt();
            match name {
                Ok(name) => {
                    let new_mode = NewModeBuilder::default().summary(name).build().unwrap();
                    // In this menu we are only editing mode definitions, not changing the current mode,
                    // so we don't need the newly created mode value. However, the data layer sends it
                    // back over a oneshot channel and will panic if nobody is listening, so we create
                    // a oneshot, pass the sender, and explicitly await and discard the result.
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    send_to_data_storage_layer
                        .send(DataLayerCommands::NewMode(new_mode, sender))
                        .await
                        .unwrap();
                    let _ = receiver.await;

                    Box::pin(configure_modes(send_to_data_storage_layer)).await
                }
                Err(InquireError::OperationCanceled) => {
                    Box::pin(configure_modes(send_to_data_storage_layer)).await
                }
                Err(InquireError::OperationInterrupted) => Err(()),
                Err(_) => {
                    todo!()
                }
            }
        }
        Ok(ConfigureModesOptions::Mode(mode)) => {
            let options = vec![
                ConfigureModesOptionsSelected::AddWithParent(mode),
                ConfigureModesOptionsSelected::EditName(mode),
                ConfigureModesOptionsSelected::Back,
                ConfigureModesOptionsSelected::Done,
            ];

            println!();
            let selection = Select::new("Select an option", options).prompt();
            match selection {
                Ok(ConfigureModesOptionsSelected::AddWithParent(parent)) => {
                    let name = inquire::Text::new("Enter the name of the new mode").prompt();
                    match name {
                        Ok(name) => {
                            let new_mode = NewModeBuilder::default()
                                .summary(name)
                                .parent_mode(Some(parent.get_surreal_id().clone()))
                                .build()
                                .unwrap();
                            // As above, this path is configuring modes, not switching the current mode,
                            // so we only need to listen on the oneshot to prevent the data layer from
                            // panicking when it sends back the created mode.
                            let (sender, receiver) = tokio::sync::oneshot::channel();
                            send_to_data_storage_layer
                                .send(DataLayerCommands::NewMode(new_mode, sender))
                                .await
                                .unwrap();
                            let _ = receiver.await;

                            Box::pin(configure_modes(send_to_data_storage_layer)).await
                        }
                        Err(InquireError::OperationCanceled) => {
                            Box::pin(configure_modes(send_to_data_storage_layer)).await
                        }
                        Err(InquireError::OperationInterrupted) => Err(()),
                        Err(_) => {
                            todo!()
                        }
                    }
                }
                Ok(ConfigureModesOptionsSelected::EditName(mode)) => {
                    let name = inquire::Text::new("Enter the new name of the mode")
                        .with_default(mode.get_name())
                        .prompt();
                    match name {
                        Ok(name) => {
                            send_to_data_storage_layer
                                .send(DataLayerCommands::UpdateModeSummary(
                                    mode.get_surreal_id().clone(),
                                    name,
                                ))
                                .await
                                .unwrap();

                            Box::pin(configure_modes(send_to_data_storage_layer)).await
                        }
                        Err(InquireError::OperationCanceled) => {
                            Box::pin(configure_modes(send_to_data_storage_layer)).await
                        }
                        Err(InquireError::OperationInterrupted) => Err(()),
                        Err(_) => {
                            todo!()
                        }
                    }
                }
                Ok(ConfigureModesOptionsSelected::Done) => Ok(()),
                Ok(ConfigureModesOptionsSelected::Back) | Err(InquireError::OperationCanceled) => {
                    Box::pin(configure_modes(send_to_data_storage_layer)).await
                }
                Err(InquireError::OperationInterrupted) => Err(()),
                Err(_) => {
                    todo!()
                }
            }
        }
        Ok(ConfigureModesOptions::Done) => Ok(()),
        Err(InquireError::OperationCanceled) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(_) => {
            todo!()
        }
    }
}
