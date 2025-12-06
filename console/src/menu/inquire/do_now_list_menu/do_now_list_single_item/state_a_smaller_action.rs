use std::{cmp::Ordering, fmt::Display, iter::once};

use chrono::Utc;
use inquire::{InquireError, Select};
use itertools::chain;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::display_item_node::DisplayItemNode,
    menu::inquire::select_higher_importance_than_this::select_higher_importance_than_this,
    node::{Filter, item_node::ItemNode, item_status::ItemStatus},
};

use crate::menu::inquire::default_select_page_size;

use super::{
    DisplayFormat, ItemTypeSelection,
    new_item::NewDependency,
    urgency_plan::{AddOrRemove, prompt_for_dependencies_and_urgency_plan},
};

pub(crate) enum SelectAnItemSortingOrder {
    MotivationsFirst,
    NewestFirst,
}

enum ChildItem<'e> {
    CreateNewItem,
    ItemNode(DisplayItemNode<'e>),
}

impl Display for ChildItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChildItem::CreateNewItem => write!(f, "🗬   Create New Item"),
            ChildItem::ItemNode(item) => write!(f, "{}", item),
        }
    }
}

pub(crate) async fn select_an_item<'a>(
    dont_show_these_items: Vec<&Item<'_>>,
    sorting_order: SelectAnItemSortingOrder,
    calculated_data: &'a CalculatedData,
) -> Result<Option<&'a ItemStatus<'a>>, ()> {
    let items_status = calculated_data.get_items_status();
    let active_items = items_status
        .iter()
        .filter(|(_, x)| {
            !dont_show_these_items.iter().any(|y| x.get_item() == *y) && !x.is_finished()
        })
        .map(|(_, v)| v);
    let mut existing_items = active_items
        .map(|x| DisplayItemNode::new(x.get_item_node(), Filter::Active, DisplayFormat::SingleLine))
        .collect::<Vec<_>>();
    match sorting_order {
        SelectAnItemSortingOrder::MotivationsFirst => existing_items.sort_by(|a, b| {
            if a.is_type_motivation() {
                if b.is_type_motivation() {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            } else if a.is_type_goal() {
                if b.is_type_motivation() {
                    Ordering::Greater
                } else if b.is_type_goal() {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            } else if b.is_type_motivation() || b.is_type_goal() {
                Ordering::Greater
            } else if a.get_type() == b.get_type() {
                Ordering::Equal
            } else {
                Ordering::Less
            }
            .then_with(|| a.get_item().get_summary().cmp(b.get_item().get_summary()))
            .then_with(|| a.get_created().cmp(b.get_created()).reverse())
        }),
        SelectAnItemSortingOrder::NewestFirst => {
            existing_items.sort_by(|a, b| a.get_created().cmp(b.get_created()).reverse())
        }
    }
    let list = chain!(
        once(ChildItem::CreateNewItem),
        existing_items.into_iter().map(ChildItem::ItemNode)
    )
    .collect::<Vec<_>>();
    let selection = Select::new(
        "Select an existing item from this list of all items or create a new item, type to search|",
        list,
    )
    .with_page_size(default_select_page_size())
    .prompt();
    match selection {
        Ok(ChildItem::CreateNewItem) => Ok(None),
        Ok(ChildItem::ItemNode(selected_item)) => {
            Ok(items_status.get(selected_item.get_item().get_surreal_record_id()))
        }
        Err(InquireError::OperationCanceled | InquireError::InvalidConfiguration(_)) => Ok(None),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) async fn state_a_smaller_action(
    selected_item: &ItemNode<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let calculated_data = CalculatedData::new_from_base_data(base_data);
    let selection = select_an_item(
        vec![selected_item.get_item()],
        SelectAnItemSortingOrder::NewestFirst,
        &calculated_data,
    )
    .await;

    match selection {
        Ok(Some(child)) => {
            let parent = selected_item;
            let child: &Item<'_> = child.get_item();

            let higher_importance_than_this = if parent.has_children(Filter::Active) {
                let items = parent
                    .get_children(Filter::Active)
                    .map(|x| x.get_item())
                    .collect::<Vec<_>>();
                select_higher_importance_than_this(&items, None)
            } else {
                None
            };
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentItemWithExistingItem {
                    child: child.get_surreal_record_id().clone(),
                    parent: parent.get_surreal_record_id().clone(),
                    higher_importance_than_this,
                })
                .await
                .unwrap();

            Ok(())
        }
        Ok(None) => {
            state_a_child_action_new_item(
                selected_item,
                calculated_data.get_base_data(),
                send_to_data_storage_layer,
            )
            .await
        }
        Err(()) => Err(()),
    }
}

pub(crate) async fn state_a_child_action_new_item(
    selected_item: &ItemNode<'_>,
    base_data: &BaseData,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = ItemTypeSelection::create_list();

    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ItemTypeSelection::NormalHelp) => {
            ItemTypeSelection::print_normal_help();
            Box::pin(state_a_child_action_new_item(
                selected_item,
                base_data,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(item_type_selection) => {
            let mut new_item = item_type_selection.create_new_item_prompt_user_for_summary();
            let higher_importance_than_this = if selected_item.has_children(Filter::Active) {
                let items = selected_item
                    .get_children(Filter::Active)
                    .map(|x| x.get_item())
                    .collect::<Vec<_>>();
                select_higher_importance_than_this(&items, None)
            } else {
                None
            };
            let parent = selected_item;

            let (dependencies, urgency_plan) = prompt_for_dependencies_and_urgency_plan(
                None,
                base_data,
                send_to_data_storage_layer,
            )
            .await;
            let dependencies = dependencies.into_iter().map(|a|
                match a {
                    AddOrRemove::AddExisting(b) => NewDependency::Existing(b),
                    AddOrRemove::AddNewEvent(new_event) => NewDependency::NewEvent(new_event),
                    AddOrRemove::RemoveExisting(_) => unreachable!("You are adding a new item there is nothing to remove so this case will never be hit"),
                }).collect::<Vec<_>>();
            new_item.dependencies = dependencies;
            new_item.urgency_plan = Some(urgency_plan);

            send_to_data_storage_layer
                .send(DataLayerCommands::ParentItemWithANewChildItem {
                    child: new_item,
                    parent: parent.get_surreal_record_id().clone(),
                    higher_importance_than_this,
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::OperationCanceled) => todo!(),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => todo!("Unexpected {}", err),
    }
}
