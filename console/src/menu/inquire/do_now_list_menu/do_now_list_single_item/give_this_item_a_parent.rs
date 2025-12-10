use std::fmt;

use chrono::Utc;
use inquire::{InquireError, Select};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item},
    calculated_data::parent_lookup::ParentLookup,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::display_item_node::{DisplayItemNode, DisplayItemNodeSortExt},
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
    let nodes = active_items
        .iter()
        .filter(|x| x.get_surreal_record_id() != parent_this.get_surreal_record_id())
        .map(|item| ItemNode::new(item, all_items, &parent_lookup, all_events, time_spent_log))
        //Collect the ItemNodes because they need a place to be so they don't go out of scope as DisplayItemNode
        //only takes a reference.
        .collect::<Vec<_>>();

    let mut display_nodes = nodes
        .iter()
        .map(|node| DisplayItemNode::new(node, Filter::Active, DisplayFormat::SingleLine))
        .collect::<Vec<_>>();
    display_nodes.sort_motivations_first_by_summary_then_created();

    let mut list = Vec::new();
    if show_finish_option {
        list.push(ParentItem::CreateNewItem);
        list.push(ParentItem::FinishItem);
    }
    for node in display_nodes.into_iter() {
        list.push(ParentItem::ItemNode(node));
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

#[cfg(test)]
mod tests {
    use crate::{
        calculated_data::parent_lookup::ParentLookup,
        data_storage::surrealdb_layer::{
            surreal_item::{
                SurrealHowMuchIsInMyControl, SurrealItemBuilder, SurrealItemType,
                SurrealMotivationKind,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        display::display_item_node::{DisplayItemNode, DisplayItemNodeSortExt},
        node::{Filter, item_node::ItemNode},
    };
    use super::DisplayFormat;
    use chrono::Utc;
    use surrealdb::RecordId;

    #[test]
    fn motivations_are_sorted_alphabetically_when_selecting_a_parent() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "1").into()))
                .summary("Zebra motivation")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "2").into()))
                .summary("Apple motivation")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "3").into()))
                .summary("Goal item")
                .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::NotSet))
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let events = surreal_tables.make_events();
        let time_spent_log = surreal_tables.make_time_spent_log().collect::<Vec<_>>();

        let zebra_id: RecordId = ("surreal_item", "1").into();
        let apple_id: RecordId = ("surreal_item", "2").into();
        let goal_id: RecordId = ("surreal_item", "3").into();

        let zebra_node = ItemNode::new(
            items.get(&zebra_id).expect("Item exists"),
            &items,
            &parent_lookup,
            &events,
            &time_spent_log,
        );
        let apple_node = ItemNode::new(
            items.get(&apple_id).expect("Item exists"),
            &items,
            &parent_lookup,
            &events,
            &time_spent_log,
        );
        let goal_node = ItemNode::new(
            items.get(&goal_id).expect("Item exists"),
            &items,
            &parent_lookup,
            &events,
            &time_spent_log,
        );

        let nodes = vec![zebra_node, apple_node, goal_node];

        let mut display_nodes = nodes
            .iter()
            .map(|node| DisplayItemNode::new(node, Filter::Active, DisplayFormat::SingleLine))
            .collect::<Vec<_>>();

        display_nodes.sort_motivations_first_by_summary_then_created();

        let summaries: Vec<&str> = display_nodes
            .iter()
            .map(|node| node.get_item().get_summary())
            .collect();
        assert_eq!(
            summaries,
            vec!["Apple motivation", "Zebra motivation", "Goal item"]
        );
    }
}
