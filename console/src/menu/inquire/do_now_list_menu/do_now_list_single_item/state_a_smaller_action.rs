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
use crate::data_storage::surrealdb_layer::surreal_item::SurrealItemType;

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

fn item_type_sort_order(item_type: &SurrealItemType) -> u8 {
    // Assign a numeric value to each variant for consistent ordering
    match item_type {
        SurrealItemType::Undeclared => 0,
        SurrealItemType::Action => 1,
        SurrealItemType::Goal(_) => 2,
        SurrealItemType::IdeaOrThought => 3,
        SurrealItemType::Motivation(_) => 4,
        SurrealItemType::PersonOrGroup => 5,
    }
}

fn sort_items_motivations_first(items: &mut [DisplayItemNode<'_>]) {
    items.sort_by(|a, b| {
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
        } else {
            // For items that are neither motivations nor goals, compare by type
            // to ensure a stable ordering (satisfying the antisymmetric property)
            item_type_sort_order(a.get_type()).cmp(&item_type_sort_order(b.get_type()))
        }
        .then_with(|| a.get_item().get_summary().cmp(b.get_item().get_summary()))
        .then_with(|| a.get_created().cmp(b.get_created()).reverse())
    });
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
        SelectAnItemSortingOrder::MotivationsFirst => {
            sort_items_motivations_first(&mut existing_items)
        }
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

#[cfg(test)]
mod tests {
    use super::sort_items_motivations_first;
    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            surreal_item::{
                SurrealHowMuchIsInMyControl, SurrealItemBuilder, SurrealItemType,
                SurrealMotivationKind,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        display::display_item_node::DisplayItemNode,
        node::Filter,
    };
    use chrono::Utc;
    use surrealdb::RecordId;

    use super::DisplayFormat;

    #[test]
    fn items_are_sorted_alphabetically_within_type_groups() {
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
                .summary("Zebra goal")
                .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::NotSet))
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "4").into()))
                .summary("Apple goal")
                .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::NotSet))
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "5").into()))
                .summary("Zebra action")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "6").into()))
                .summary("Apple action")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();
        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        let zebra_motivation_id: RecordId = ("surreal_item", "1").into();
        let apple_motivation_id: RecordId = ("surreal_item", "2").into();
        let zebra_goal_id: RecordId = ("surreal_item", "3").into();
        let apple_goal_id: RecordId = ("surreal_item", "4").into();
        let zebra_action_id: RecordId = ("surreal_item", "5").into();
        let apple_action_id: RecordId = ("surreal_item", "6").into();

        let items_status = calculated_data.get_items_status();

        let mut display_items = vec![
            DisplayItemNode::new(
                items_status
                    .get(&zebra_motivation_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&apple_motivation_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&zebra_goal_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&apple_goal_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&zebra_action_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&apple_action_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
        ];

        sort_items_motivations_first(&mut display_items);

        let summaries: Vec<&str> = display_items
            .iter()
            .map(|item| item.get_item().get_summary())
            .collect();
        assert_eq!(
            summaries,
            vec![
                "Apple motivation",
                "Zebra motivation",
                "Apple goal",
                "Zebra goal",
                "Apple action",
                "Zebra action"
            ]
        );
    }

    #[test]
    fn different_item_types_are_sorted_consistently() {
        // This test ensures that the sorting function satisfies the antisymmetric property
        // when comparing different item types (neither motivations nor goals)
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "1").into()))
                .summary("Action item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "2").into()))
                .summary("Idea item")
                .item_type(SurrealItemType::IdeaOrThought)
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "3").into()))
                .summary("Undeclared item")
                .item_type(SurrealItemType::Undeclared)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();
        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        let action_id: RecordId = ("surreal_item", "1").into();
        let idea_id: RecordId = ("surreal_item", "2").into();
        let undeclared_id: RecordId = ("surreal_item", "3").into();

        let items_status = calculated_data.get_items_status();

        let mut display_items = vec![
            DisplayItemNode::new(
                items_status
                    .get(&action_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&idea_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
            DisplayItemNode::new(
                items_status
                    .get(&undeclared_id)
                    .expect("Item exists")
                    .get_item_node(),
                Filter::Active,
                DisplayFormat::SingleLine,
            ),
        ];

        sort_items_motivations_first(&mut display_items);

        let summaries: Vec<&str> = display_items
            .iter()
            .map(|item| item.get_item().get_summary())
            .collect();

        // Items should be sorted by type (Undeclared < Action < IdeaOrThought)
        // then alphabetically within each type
        assert_eq!(
            summaries,
            vec!["Undeclared item", "Action item", "Idea item"]
        );
    }
}

