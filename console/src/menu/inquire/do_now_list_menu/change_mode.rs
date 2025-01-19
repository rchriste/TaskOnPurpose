use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    iter::once,
};

use inquire::{InquireError, Select, Text};
use itertools::chain;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    base_data::mode::Mode,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_current_mode::NewCurrentMode,
    },
    display::display_mode_node::DisplayModeNode,
    new_mode::NewModeBuilder,
    node::mode_node::ModeNode,
    systems::do_now_list::current_mode::CurrentMode,
};

use super::{CalculatedData, DisplayFormat};

pub(crate) enum InTheModeChoices<'e> {
    AddNewMode,
    SelectExistingMode(&'e ModeNode<'e>),
    ClearModeSelection,
}

impl Display for InTheModeChoices<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddNewMode => write!(f, "➕ Add New Mode"),
            Self::SelectExistingMode(mode_node) => write!(
                f,
                "{}",
                DisplayModeNode::new(mode_node, DisplayFormat::SingleLine)
            ),
            Self::ClearModeSelection => write!(f, "⌧ Clear Mode"),
        }
    }
}

enum DetailsMenu {
    ReturnToDoNowList,
    Rename,
    EditWhatIsInTheMode,
}

impl Display for DetailsMenu {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReturnToDoNowList => write!(f, "👆🏻 Return to Do Now List"),
            Self::Rename => write!(f, "✍ Rename Mode"),
            Self::EditWhatIsInTheMode => write!(f, "🗃️ Edit What is in the Mode"),
        }
    }
}

enum ParentChoice<'e> {
    NoParent,
    NewParent,
    Parent(&'e ModeNode<'e>),
}

impl Display for ParentChoice<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoParent => write!(f, "No Parent"),
            Self::NewParent => write!(f, "New Parent"),
            Self::Parent(mode_node) => write!(
                f,
                "{}",
                DisplayModeNode::new(mode_node, DisplayFormat::SingleLine)
            ),
        }
    }
}

pub(crate) async fn present_change_mode_menu(
    current_mode: &Option<CurrentMode<'_>>,
    calculated_data: &CalculatedData,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let mut mode_nodes = calculated_data.get_mode_nodes().iter().collect::<Vec<_>>();
    mode_nodes.sort_by(|a, b| {
        fn compare_chains(
            mut a_parent_chain: Vec<&Mode<'_>>,
            mut b_parent_chain: Vec<&Mode<'_>>,
        ) -> Ordering {
            let a_parent_chain_last = a_parent_chain.last();
            let b_parent_chain_last = b_parent_chain.last();
            if a_parent_chain_last.is_none() && b_parent_chain_last.is_none() {
                Ordering::Equal
            } else if a_parent_chain_last.is_none() {
                Ordering::Less
            } else if b_parent_chain_last.is_none() {
                Ordering::Greater
            } else {
                let a_parent_chain_last =
                    a_parent_chain_last.expect("Earlier if statement guarantees this is_some()");
                let b_parent_chain_last =
                    b_parent_chain_last.expect("Earlier if statement guarantees this is_some()");
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

        let a_self_parent_chain = a.create_self_parent_chain();
        let b_self_parent_chain = b.create_self_parent_chain();
        compare_chains(a_self_parent_chain, b_self_parent_chain)
    });

    let choices = chain!(
        once(InTheModeChoices::AddNewMode),
        mode_nodes
            .into_iter()
            .map(InTheModeChoices::SelectExistingMode),
        once(InTheModeChoices::ClearModeSelection)
    )
    .collect::<Vec<_>>();
    let default_choice = choices
        .iter()
        .enumerate()
        .find(|(i, x)| match x {
            InTheModeChoices::SelectExistingMode(mode_node) => match current_mode {
                Some(current_mode) => {
                    mode_node.get_surreal_id() == current_mode.get_mode().get_surreal_id()
                }
                None => false,
            },
            InTheModeChoices::AddNewMode | InTheModeChoices::ClearModeSelection => false,
        })
        .map(|(i, _)| i)
        .unwrap_or_default();

    let selection = Select::new("Select Mode to Change to", choices)
        .with_starting_cursor(default_choice)
        .prompt();

    let selected = match selection {
        Ok(InTheModeChoices::AddNewMode) => {
            let name = Text::new("Enter the name of the new mode")
                .prompt()
                .unwrap();

            let options = chain!(
                once(ParentChoice::NoParent),
                once(ParentChoice::NewParent),
                calculated_data
                    .get_mode_nodes()
                    .iter()
                    .map(ParentChoice::Parent)
            )
            .collect::<Vec<_>>();

            let parent = Select::new("Should this mode have a parent or category", options)
                .prompt()
                .unwrap();
            match parent {
                ParentChoice::NoParent => {
                    let (sender, receiver) = oneshot::channel();
                    let new_mode = NewModeBuilder::default()
                        .summary(name)
                        .build()
                        .expect("Everything required is filled out");
                    send_to_data_storage_layer
                        .send(DataLayerCommands::NewMode(new_mode, sender))
                        .await
                        .unwrap();
                    Some(receiver.await.unwrap())
                }
                ParentChoice::NewParent => todo!(),
                ParentChoice::Parent(mode_node) => {
                    let (sender, receiver) = oneshot::channel();
                    let new_mode = NewModeBuilder::default()
                        .summary(name)
                        .parent_mode(Some(mode_node.get_surreal_id().clone()))
                        .build()
                        .expect("Everything required is filled out");
                    send_to_data_storage_layer
                        .send(DataLayerCommands::NewMode(new_mode, sender))
                        .await
                        .unwrap();
                    Some(receiver.await.unwrap())
                }
            }
            //todo!("Prompt for who the parent node should be and then prompt to name the mode, then set this new mode as the current mode and bring the user to the InTheModeChoices::SelectExistingMode menu so they can choose to edit what is in and out of the mode if they would like otherwise to continue")
        }
        Ok(InTheModeChoices::ClearModeSelection) => None,
        Ok(InTheModeChoices::SelectExistingMode(mode_node)) => {
            Some(mode_node.get_surreal().clone())
        }
        Err(InquireError::OperationCanceled) => return Ok(()),
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    let mode_surreal_id = selected.as_ref().map(|x| x.id.as_ref().unwrap().clone());
    let new_current_mode = NewCurrentMode::new(mode_surreal_id);
    send_to_data_storage_layer
        .send(DataLayerCommands::SetCurrentMode(new_current_mode))
        .await
        .unwrap();

    match selected {
        None => Ok(()),
        Some(selected) => {
            let selection = Select::new(
                "Select one",
                vec![
                    DetailsMenu::ReturnToDoNowList,
                    DetailsMenu::EditWhatIsInTheMode,
                    DetailsMenu::Rename,
                ],
            )
            .prompt();
            match selection {
                Ok(DetailsMenu::ReturnToDoNowList) => Ok(()),
                Ok(DetailsMenu::EditWhatIsInTheMode) => {
                    todo!("Present the menu to edit what is in and out of the mode, core work, non-core work, and explicitly out of scope work")
                }
                Ok(DetailsMenu::Rename) => {
                    let mode = Mode::new(&selected);
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

                            Ok(())
                        }
                        Err(InquireError::OperationCanceled) => {
                            Ok(()) //Just let it fall back to the normal menu
                        }
                        Err(InquireError::OperationInterrupted) => Err(()),
                        Err(err) => {
                            panic!("Unexpected error, try restarting the terminal: {}", err)
                        }
                    }
                }
                Err(InquireError::OperationCanceled) => {
                    Ok(()) //Just let it fall back to the normal menu
                }
                Err(InquireError::OperationInterrupted) => Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }
    }
}
