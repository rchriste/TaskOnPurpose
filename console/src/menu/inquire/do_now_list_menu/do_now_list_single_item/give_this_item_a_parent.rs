use std::{cmp::Ordering, fmt};

use chrono::Utc;
use inquire::{InquireError, Select};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item},
    calculated_data::parent_lookup::ParentLookup,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::display_item_node::DisplayItemNode,
    menu::inquire::{
        do_now_list_menu::do_now_list_single_item::ItemTypeSelection,
        select_higher_importance_than_this::select_higher_importance_than_this,
    },
    node::{Filter, item_node::ItemNode},
};

use crate::menu::inquire::default_select_page_size;

use super::DisplayFormat;

enum ParentItem<'e> {
    ItemNode(DisplayItemNode<'e>),
    FinishItem,
    CreateNewItem,
}

impl fmt::Display for ParentItem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParentItem::ItemNode(node) => write!(f, "{}", node),
            ParentItem::FinishItem => write!(f, "🚪Finish Item"),
            ParentItem::CreateNewItem => write!(f, "🗰 Create New Item"),
        }
    }
}

pub(crate) async fn give_this_item_a_parent(
    parent_this: &Item<'_>,
    show_finish_option: bool,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let all_items = base_data.get_items();
    let parent_lookup = ParentLookup::new(all_items);
    let active_items = base_data.get_active_items();
    let all_events = base_data.get_events();
    let time_spent_log = base_data.get_time_spent_log();
    let mut nodes = active_items
        .iter()
        .filter(|x| x.get_surreal_record_id() != parent_this.get_surreal_record_id())
        .map(|item| ItemNode::new(item, all_items, &parent_lookup, all_events, time_spent_log))
        //Collect the ItemNodes because they need a place to be so they don't go out of scope as DisplayItemNode
        //only takes a reference.
        .collect::<Vec<_>>();

    nodes.sort_by(|a, b| {
        //Motivational items first, then goals, then everything else.
        if a.is_type_motivation() && !b.is_type_motivation() {
            Ordering::Less
        } else if !a.is_type_motivation() && b.is_type_motivation() {
            Ordering::Greater
        } else if a.is_type_goal() && !b.is_type_goal() {
            Ordering::Less
        } else if !a.is_type_goal() && b.is_type_goal() {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });

    let mut list = Vec::new();
    if show_finish_option {
        list.push(ParentItem::CreateNewItem);
        list.push(ParentItem::FinishItem);
    }
    for node in nodes.iter() {
        list.push(ParentItem::ItemNode(DisplayItemNode::new(
            node,
            Filter::Active,
            DisplayFormat::SingleLine,
        )));
    }

    let selection = Select::new(
        "Type to search, select an existing reason, or create a new item|",
        list,
    )
    .with_page_size(default_select_page_size())
    .prompt();
    match selection {
        Ok(ParentItem::FinishItem) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::FinishItem {
                    item: parent_this.get_surreal_record_id().clone(),
                    when_finished: (*parent_this.get_now()).into(),
                })
                .await
                .unwrap();

            Ok(())
        }
        Ok(ParentItem::ItemNode(parent)) => {
            let parent: &ItemNode<'_> = parent.get_item_node();

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
                    child: parent_this.get_surreal_record_id().clone(),
                    parent: parent.get_surreal_record_id().clone(),
                    higher_importance_than_this,
                })
                .await
                .unwrap();
            Ok(())
        }
        Ok(ParentItem::CreateNewItem) | Err(InquireError::InvalidConfiguration(_)) => {
            parent_to_a_goal_or_motivation_new_goal_or_motivation(
                parent_this,
                send_to_data_storage_layer,
            )
            .await
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

async fn parent_to_a_goal_or_motivation_new_goal_or_motivation(
    parent_this: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = ItemTypeSelection::create_list();
    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ItemTypeSelection::NormalHelp) => {
            ItemTypeSelection::print_normal_help();
            Box::pin(parent_to_a_goal_or_motivation_new_goal_or_motivation(
                parent_this,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(item_type_selection) => {
            let new_item = item_type_selection.create_new_item_prompt_user_for_summary();
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentNewItemWithAnExistingChildItem {
                    child: parent_this.get_surreal_record_id().clone(),
                    parent_new_item: new_item,
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::OperationCanceled) => {
            todo!("I need to go back to what first called this");
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}
