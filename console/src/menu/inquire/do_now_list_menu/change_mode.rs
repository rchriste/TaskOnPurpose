use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    iter::once,
};

use chrono::Utc;
use inquire::{InquireError, Select, Text};
use itertools::chain;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    base_data::{BaseData, mode::Mode},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::{DataLayerCommands, ScopeModeCommand},
        surreal_current_mode::NewCurrentMode,
        surreal_item::SurrealUrgencyNoData,
        surreal_mode::SurrealScope,
        surreal_tables::SurrealTables,
    },
    display::{display_item_node::DisplayItemNode, display_mode_node::DisplayModeNode},
    menu::inquire::do_now_list_menu::do_now_list_single_item::state_a_smaller_action::{
        SelectAnItemSortingOrder, ShowCreateNewItem, select_an_item,
    },
    new_mode::NewModeBuilder,
    node::{Filter, item_status::ItemStatus, mode_node::ModeNode},
    systems::do_now_list::current_mode::CurrentMode,
};

use super::DisplayFormat;

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
            match (a_parent_chain.last(), b_parent_chain.last()) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(a_last), Some(b_last)) => {
                    let ordering = a_last.get_name().cmp(b_last.get_name());
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
        .find(|(_, x)| match x {
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

    let selected_mode_id = match selection {
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
                    let surreal_mode = receiver.await.unwrap();
                    Some(
                        surreal_mode
                            .id
                            .as_ref()
                            .expect("Newly created mode is in the database")
                            .clone(),
                    )
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
                    let surreal_mode = receiver.await.unwrap();
                    Some(
                        surreal_mode
                            .id
                            .as_ref()
                            .expect("Newly created mode is in the database")
                            .clone(),
                    )
                }
            }
            //todo!("Prompt for who the parent node should be and then prompt to name the mode, then set this new mode as the current mode and bring the user to the InTheModeChoices::SelectExistingMode menu so they can choose to edit what is in and out of the mode if they would like otherwise to continue")
        }
        Ok(InTheModeChoices::ClearModeSelection) => None,
        Ok(InTheModeChoices::SelectExistingMode(mode_node)) => {
            Some(mode_node.get_surreal_id().clone())
        }
        Err(InquireError::OperationCanceled) => return Ok(()),
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    let new_current_mode = NewCurrentMode::new(selected_mode_id.clone());
    send_to_data_storage_layer
        .send(DataLayerCommands::SetCurrentMode(new_current_mode))
        .await
        .unwrap();

    match selected_mode_id {
        None => Ok(()),
        Some(mode_id) => {
            // IMPORTANT: We fetch fresh CalculatedData here (instead of reusing the incoming
            // CalculatedData) so that newly created modes are included in the mode tree and we
            // can work with a proper ModeNode, consistent with the rest of the system.
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            let now = Utc::now();
            let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
            let calculated_data = CalculatedData::new_from_base_data(base_data);

            let mode_nodes = calculated_data.get_mode_nodes();
            let mode_node = mode_nodes
                .iter()
                .find(|node| node.get_surreal_id() == &mode_id)
                .expect("Current mode should always exist in CalculatedData after refresh");

            let mode = mode_node.get_mode();
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
                    present_edit_mode_scope_menu(mode, send_to_data_storage_layer).await
                }
                Ok(DetailsMenu::Rename) => {
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

enum EditScopeMenu {
    AddCore,
    AddNonCore,
    AddOutOfScope,
    RemoveCore,
    RemoveNonCore,
    RemoveOutOfScope,
    Done,
}

impl Display for EditScopeMenu {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EditScopeMenu::AddCore => write!(f, "➕ Add items as 🏢Core🏢 in this mode"),
            EditScopeMenu::AddNonCore => write!(f, "➕ Add items as 🧹Non-Core🧹 in this mode"),
            EditScopeMenu::AddOutOfScope => {
                write!(f, "➕ Add items as explicitly 🚫Out of Scope🚫")
            }
            EditScopeMenu::RemoveCore => {
                write!(f, "🚫 Remove items as 🏢Core🏢 in this mode")
            }
            EditScopeMenu::RemoveNonCore => {
                write!(f, "🚫 Remove items as 🧹Non-Core🧹 in this mode")
            }
            EditScopeMenu::RemoveOutOfScope => {
                write!(f, "🚫 Remove items as explicitly 🚫Out of Scope🚫")
            }
            EditScopeMenu::Done => write!(f, "⬅ Return to the Do Now list"),
        }
    }
}

#[derive(Clone)]
struct ScopeItemForDisplay<'s> {
    item_id: surrealdb::opt::RecordId,
    display: Option<DisplayItemNode<'s>>,
}

impl Display for ScopeItemForDisplay<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.display {
            Some(node) => write!(f, "{}", node),
            None => write!(f, "<missing item>"),
        }
    }
}

enum SelectAnotherItem {
    SelectAnother,
    Done,
}

impl Display for SelectAnotherItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectAnotherItem::SelectAnother => write!(f, "Select another"),
            SelectAnotherItem::Done => write!(f, "Done"),
        }
    }
}

async fn select_items_for_mode(
    calculated_data: &CalculatedData,
    excluded_ids: &[surrealdb::opt::RecordId],
) -> Result<Vec<surrealdb::opt::RecordId>, ()> {
    let items = calculated_data.get_items();

    let mut result: Vec<surrealdb::opt::RecordId> = Vec::new();
    let mut already_selected: Vec<&ItemStatus<'_>> = Vec::new();

    loop {
        let mut dont_show_these_items = already_selected
            .iter()
            .map(|status| status.get_item())
            .collect::<Vec<_>>();
        // Also exclude any items that are already in one of the mode's scope lists
        for excluded in excluded_ids {
            if let Some(item) = items.get(excluded) {
                dont_show_these_items.push(item);
            }
        }
        let selected = select_an_item(
            dont_show_these_items,
            SelectAnItemSortingOrder::MotivationsFirst,
            calculated_data,
            ShowCreateNewItem::No,
        )
        .await;
        let selected = match selected {
            Ok(Some(status)) => status,
            Ok(None) => break,
            Err(()) => return Err(()),
        };

        result.push(selected.get_surreal_record_id().clone());
        already_selected.push(selected);

        let select_another = Select::new(
            "Do you want to select another item?",
            vec![SelectAnotherItem::SelectAnother, SelectAnotherItem::Done],
        )
        .prompt()
        .unwrap();
        match select_another {
            SelectAnotherItem::SelectAnother => {
                //continue loop
            }
            SelectAnotherItem::Done => break,
        }
    }

    Ok(result)
}

async fn present_edit_mode_scope_menu(
    mode: &Mode<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let mode_id = mode.get_surreal_id().clone();

    loop {
        // Always show the current scope before prompting for an action.
        let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
            .await
            .unwrap();
        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let item_statuses = calculated_data.get_items_status();

        // Refresh the mode information from CalculatedData so we always show what is actually
        // persisted in the database (including any changes made in previous iterations).
        let mode_nodes = calculated_data.get_mode_nodes();
        let mode_node = mode_nodes
            .iter()
            .find(|node| node.get_surreal_id() == &mode_id)
            .expect("Mode should always exist in CalculatedData after refresh");
        let mode = mode_node.get_mode();
        let core_in_scope = mode.get_core_in_scope();
        let non_core_in_scope = mode.get_non_core_in_scope();
        let explicitly_out_of_scope_items = mode.get_explicitly_out_of_scope_items();

        println!();
        println!("Editing what is in and out of mode: {}", mode.get_name());
        println!();

        println!("🏢 Core in scope for this mode:");
        if core_in_scope.is_empty() {
            println!("\t(none)");
        } else {
            for scope in core_in_scope {
                let status = item_statuses
                    .values()
                    .find(|s| s.get_surreal_record_id() == &scope.for_item);
                match status {
                    Some(status) => {
                        let node = status.get_item_node();
                        println!(
                            "{}",
                            DisplayItemNode::new(
                                node,
                                Filter::Active,
                                DisplayFormat::MultiLineTree
                            )
                        );
                    }
                    None => println!("\t- <missing item>"),
                }
            }
        }

        println!();
        println!("🧹 Non-Core in scope for this mode:");
        if non_core_in_scope.is_empty() {
            println!("\t(none)");
        } else {
            for scope in non_core_in_scope {
                let status = item_statuses
                    .values()
                    .find(|s| s.get_surreal_record_id() == &scope.for_item);
                match status {
                    Some(status) => {
                        let node = status.get_item_node();
                        println!(
                            "{}",
                            DisplayItemNode::new(
                                node,
                                Filter::Active,
                                DisplayFormat::MultiLineTree
                            )
                        );
                    }
                    None => println!("\t- <missing item>"),
                }
            }
        }

        println!();
        println!("🚫 Explicitly Out of Scope items for this mode:");
        if explicitly_out_of_scope_items.is_empty() {
            println!("\t(none)");
        } else {
            for record_id in explicitly_out_of_scope_items {
                let status = item_statuses
                    .values()
                    .find(|s| s.get_surreal_record_id() == record_id);
                match status {
                    Some(status) => {
                        let node = status.get_item_node();
                        println!(
                            "{}",
                            DisplayItemNode::new(
                                node,
                                Filter::Active,
                                DisplayFormat::MultiLineTree
                            )
                        );
                    }
                    None => println!("\t- <missing item>"),
                }
            }
        }

        println!();

        let selection = Select::new(
            "What would you like to change?",
            vec![
                EditScopeMenu::AddCore,
                EditScopeMenu::AddNonCore,
                EditScopeMenu::AddOutOfScope,
                EditScopeMenu::RemoveCore,
                EditScopeMenu::RemoveNonCore,
                EditScopeMenu::RemoveOutOfScope,
                EditScopeMenu::Done,
            ],
        )
        .prompt();

        match selection {
            Ok(EditScopeMenu::AddCore) => {
                let excluded_ids = core_in_scope
                    .iter()
                    .map(|s| s.for_item.clone())
                    .chain(non_core_in_scope.iter().map(|s| s.for_item.clone()))
                    .chain(explicitly_out_of_scope_items.iter().cloned())
                    .collect::<Vec<_>>();
                let selected_items = select_items_for_mode(&calculated_data, &excluded_ids).await?;
                for item_id in selected_items {
                    let scope = SurrealScope {
                        for_item: item_id.clone(),
                        is_importance_in_scope: true,
                        urgencies_to_include: SurrealUrgencyNoData::all(),
                    };

                    send_to_data_storage_layer
                        .send(DataLayerCommands::DeclareScopeForMode(
                            ScopeModeCommand::AddCore {
                                mode: mode_id.clone(),
                                scope: scope.clone(),
                            },
                        ))
                        .await
                        .unwrap();
                }
            }
            Ok(EditScopeMenu::AddNonCore) => {
                let excluded_ids = core_in_scope
                    .iter()
                    .map(|s| s.for_item.clone())
                    .chain(non_core_in_scope.iter().map(|s| s.for_item.clone()))
                    .chain(explicitly_out_of_scope_items.iter().cloned())
                    .collect::<Vec<_>>();
                let selected_items = select_items_for_mode(&calculated_data, &excluded_ids).await?;
                for item_id in selected_items {
                    let scope = SurrealScope {
                        for_item: item_id.clone(),
                        is_importance_in_scope: true,
                        urgencies_to_include: SurrealUrgencyNoData::all(),
                    };

                    send_to_data_storage_layer
                        .send(DataLayerCommands::DeclareScopeForMode(
                            ScopeModeCommand::AddNonCore {
                                mode: mode_id.clone(),
                                scope: scope.clone(),
                            },
                        ))
                        .await
                        .unwrap();
                }
            }
            Ok(EditScopeMenu::AddOutOfScope) => {
                let excluded_ids = core_in_scope
                    .iter()
                    .map(|s| s.for_item.clone())
                    .chain(non_core_in_scope.iter().map(|s| s.for_item.clone()))
                    .chain(explicitly_out_of_scope_items.iter().cloned())
                    .collect::<Vec<_>>();
                let selected_items = select_items_for_mode(&calculated_data, &excluded_ids).await?;
                for item_id in selected_items {
                    send_to_data_storage_layer
                        .send(DataLayerCommands::DeclareScopeForMode(
                            ScopeModeCommand::AddExplicitlyOutOfScope {
                                mode: mode_id.clone(),
                                item: item_id.clone(),
                            },
                        ))
                        .await
                        .unwrap();
                }
            }
            Ok(EditScopeMenu::RemoveCore) => {
                if core_in_scope.is_empty() {
                    println!("There are no Core items to remove.");
                    continue;
                }

                let options: Vec<ScopeItemForDisplay> = core_in_scope
                    .iter()
                    .map(|scope| {
                        let display = item_statuses
                            .values()
                            .find(|status| status.get_surreal_record_id() == &scope.for_item)
                            .map(|status| {
                                DisplayItemNode::new(
                                    status.get_item_node(),
                                    Filter::Active,
                                    DisplayFormat::SingleLine,
                                )
                            });
                        ScopeItemForDisplay {
                            item_id: scope.for_item.clone(),
                            display,
                        }
                    })
                    .collect();

                let selection =
                    Select::new("Select a Core in-scope item to remove", options.clone()).prompt();

                match selection {
                    Ok(scope_item) => {
                        let item_id = scope_item.item_id.clone();
                        send_to_data_storage_layer
                            .send(DataLayerCommands::DeclareScopeForMode(
                                ScopeModeCommand::RemoveCore {
                                    mode: mode_id.clone(),
                                    item: item_id.clone(),
                                },
                            ))
                            .await
                            .unwrap();
                    }
                    Err(InquireError::OperationCanceled) => {}
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => {
                        panic!("Unexpected error, try restarting the terminal: {}", err)
                    }
                }
            }
            Ok(EditScopeMenu::RemoveNonCore) => {
                if non_core_in_scope.is_empty() {
                    println!("There are no Non-Core items to remove.");
                    continue;
                }

                let options: Vec<ScopeItemForDisplay> = non_core_in_scope
                    .iter()
                    .map(|scope| {
                        let display = item_statuses
                            .values()
                            .find(|status| status.get_surreal_record_id() == &scope.for_item)
                            .map(|status| {
                                DisplayItemNode::new(
                                    status.get_item_node(),
                                    Filter::Active,
                                    DisplayFormat::SingleLine,
                                )
                            });
                        ScopeItemForDisplay {
                            item_id: scope.for_item.clone(),
                            display,
                        }
                    })
                    .collect();

                let selection =
                    Select::new("Select a Non-Core in-scope item to remove", options.clone())
                        .prompt();

                match selection {
                    Ok(scope_item) => {
                        let item_id = scope_item.item_id.clone();
                        send_to_data_storage_layer
                            .send(DataLayerCommands::DeclareScopeForMode(
                                ScopeModeCommand::RemoveNonCore {
                                    mode: mode_id.clone(),
                                    item: item_id.clone(),
                                },
                            ))
                            .await
                            .unwrap();
                    }
                    Err(InquireError::OperationCanceled) => {}
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => {
                        panic!("Unexpected error, try restarting the terminal: {}", err)
                    }
                }
            }
            Ok(EditScopeMenu::RemoveOutOfScope) => {
                if explicitly_out_of_scope_items.is_empty() {
                    println!("There are no explicitly Out of Scope items to remove.");
                    continue;
                }

                let options: Vec<ScopeItemForDisplay> = explicitly_out_of_scope_items
                    .iter()
                    .map(|record_id| {
                        let display = item_statuses
                            .values()
                            .find(|status| status.get_surreal_record_id() == record_id)
                            .map(|status| {
                                DisplayItemNode::new(
                                    status.get_item_node(),
                                    Filter::Active,
                                    DisplayFormat::SingleLine,
                                )
                            });
                        ScopeItemForDisplay {
                            item_id: record_id.clone(),
                            display,
                        }
                    })
                    .collect();

                let selection = Select::new(
                    "Select an explicitly Out of Scope item to remove",
                    options.clone(),
                )
                .prompt();

                match selection {
                    Ok(scope_item) => {
                        let item_id = scope_item.item_id.clone();
                        send_to_data_storage_layer
                            .send(DataLayerCommands::DeclareScopeForMode(
                                ScopeModeCommand::RemoveExplicitlyOutOfScope {
                                    mode: mode_id.clone(),
                                    item: item_id.clone(),
                                },
                            ))
                            .await
                            .unwrap();
                    }
                    Err(InquireError::OperationCanceled) => {}
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => {
                        panic!("Unexpected error, try restarting the terminal: {}", err)
                    }
                }
            }
            Ok(EditScopeMenu::Done) => return Ok(()),
            Err(InquireError::OperationCanceled) => return Ok(()),
            Err(InquireError::OperationInterrupted) => return Err(()),
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        }
    }
}
