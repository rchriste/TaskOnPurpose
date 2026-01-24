use std::fmt::{self, Display, Formatter};

use chrono::Utc;
use inquire::Select;
use itertools::Itertools;
use surrealdb::{RecordId, sql::Datetime};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::{
        DisplayStyle,
        display_dependencies_with_item_node::DisplayDependenciesWithItemNode,
        display_item::DisplayItem,
        display_item_node::DisplayFormat,
        display_item_status::DisplayItemStatus,
        display_urgency_plan::{DisplayUrgency, DisplayUrgencyPlan},
    },
    menu::inquire::{
        default_select_page_size,
        do_now_list_menu::do_now_list_single_item::{
            give_this_item_a_parent::give_this_item_a_parent,
            state_a_smaller_action::state_a_smaller_action,
            urgency_plan::{AddOrRemove, prompt_for_dependencies, prompt_for_urgency_plan},
        },
        do_now_list_menu::pick_item_review_frequency::present_pick_item_review_frequency_menu,
        select_higher_importance_than_this::select_higher_importance_than_this,
    },
    node::{
        Filter,
        item_node::ItemNode,
        item_status::{DependencyWithItemNode, ItemStatus},
    },
};
use ahash::HashMap;

enum ReviewItemMenuChoices<'e> {
    DoneWithReview,
    UpdateRelativeImportanceDontShowSingleParent {
        parent: &'e Item<'e>,
    },
    UpdateRelativeImportanceShowParent {
        parent: &'e Item<'e>,
    },
    UpdateDependencies {
        current_item: &'e ItemStatus<'e>,
    },
    UpdateUrgencyPlan {
        current_item: &'e ItemStatus<'e>,
    },
    UpdateReviewFrequency {
        current_item: &'e ItemStatus<'e>,
    },
    FinishThisItem {
        previously_selected_item_id: RecordId,
    },
    AddNewParent,
    AddNewChild,
    GoToParent(&'e Item<'e>),
    RemoveParent(&'e Item<'e>),
    GoToChild(&'e ItemStatus<'e>),
    RemoveChild(&'e Item<'e>),
}

impl Display for ReviewItemMenuChoices<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ReviewItemMenuChoices::DoneWithReview => write!(f, "Done with this review"),
            ReviewItemMenuChoices::UpdateRelativeImportanceDontShowSingleParent { .. } => {
                write!(f, "Update relative importance of this item")
            }
            ReviewItemMenuChoices::UpdateRelativeImportanceShowParent { parent } => {
                let display_parent = DisplayItem::new(parent);
                write!(
                    f,
                    "Update relative importance of this item, for parent: {}",
                    display_parent
                )
            }
            ReviewItemMenuChoices::UpdateDependencies { current_item } => {
                let dependencies = current_item
                    .get_dependencies(Filter::Active)
                    .filter(|x| match x {
                        DependencyWithItemNode::AfterDateTime { .. }
                        | DependencyWithItemNode::UntilScheduled { .. }
                        | DependencyWithItemNode::AfterItem(_)
                        | DependencyWithItemNode::DuringItem(_)
                        | DependencyWithItemNode::AfterEvent(_) => true,
                        DependencyWithItemNode::AfterChildItem(_)
                        | DependencyWithItemNode::WaitingToBeInterrupted => false,
                    })
                    .collect::<Vec<_>>();
                let display_ready = DisplayDependenciesWithItemNode::new(
                    &dependencies,
                    Filter::Active,
                    DisplayFormat::SingleLine,
                );
                write!(
                    f,
                    "Update dependencies this item is waiting on, current setting is children plus: {}",
                    display_ready
                )
            }
            ReviewItemMenuChoices::UpdateUrgencyPlan { current_item } => {
                let display_urgency = DisplayUrgencyPlan::new(
                    current_item.get_urgency_plan(),
                    Filter::Active,
                    DisplayFormat::SingleLine,
                );
                write!(f, "Update urgency, current setting: {}", display_urgency)
            }
            ReviewItemMenuChoices::UpdateReviewFrequency { current_item } => {
                if let Some(review_frequency) =
                    current_item.get_item().get_surreal_review_frequency()
                {
                    write!(
                        f,
                        "Update review frequency, current setting: {}",
                        review_frequency
                    )
                } else {
                    write!(f, "Update review frequency, current setting: not set")
                }
            }
            ReviewItemMenuChoices::FinishThisItem { .. } => write!(f, "Finish this item"),
            ReviewItemMenuChoices::AddNewParent => write!(f, "Add new parent"),
            ReviewItemMenuChoices::AddNewChild => write!(f, "Add new child"),
            ReviewItemMenuChoices::GoToParent(item) => {
                let display_item = DisplayItem::new(item);
                write!(f, "Go to parent: {}", display_item)
            }
            ReviewItemMenuChoices::RemoveParent(item) => {
                let display_item = DisplayItem::new(item);
                write!(f, "Remove parent: {}", display_item)
            }
            ReviewItemMenuChoices::GoToChild(item) => {
                let display_item = DisplayItem::new(item.get_item());
                if let Some(urgency) = item.get_urgency_now() {
                    let display_urgency = DisplayUrgency::new(urgency, DisplayStyle::Abbreviated);
                    write!(f, "Go to child: {} {}", display_urgency, display_item)
                } else {
                    write!(f, "Go to child: {}", display_item)
                }
            }
            ReviewItemMenuChoices::RemoveChild(item) => {
                let display_item = DisplayItem::new(item);
                write!(f, "Remove child: {}", display_item)
            }
        }
    }
}

impl ReviewItemMenuChoices<'_> {
    pub(crate) fn make_list<'e>(
        current_item: &'e ItemStatus<'e>,
        previously_selected_item: &'e ItemStatus<'e>,
        all_items: &'e HashMap<&'e RecordId, ItemStatus<'e>>,
    ) -> Vec<ReviewItemMenuChoices<'e>> {
        let mut list = vec![ReviewItemMenuChoices::DoneWithReview];

        if current_item
            .get_item_node()
            .get_parents(Filter::Active)
            .count()
            == 1
        {
            let parent = current_item
                .get_item_node()
                .get_parents(Filter::Active)
                .next()
                .expect("Item is for sure there because count is 1")
                .get_item();
            list.push(
                ReviewItemMenuChoices::UpdateRelativeImportanceDontShowSingleParent { parent },
            );
        } else {
            //Note that if there is no parent then we don't show this option and that is by design
            for parent in current_item.get_item_node().get_parents(Filter::Active) {
                list.push(ReviewItemMenuChoices::UpdateRelativeImportanceShowParent {
                    parent: parent.get_item(),
                });
            }
        }

        list.push(ReviewItemMenuChoices::UpdateUrgencyPlan { current_item });
        list.push(ReviewItemMenuChoices::UpdateReviewFrequency { current_item });
        list.push(ReviewItemMenuChoices::UpdateDependencies { current_item });
        list.push(ReviewItemMenuChoices::FinishThisItem {
            previously_selected_item_id: previously_selected_item.get_surreal_record_id().clone(),
        });
        list.push(ReviewItemMenuChoices::AddNewParent);

        for parent in current_item.get_item_node().get_parents(Filter::Active) {
            list.push(ReviewItemMenuChoices::GoToParent(parent.get_item()));
        }

        for parent in current_item.get_item_node().get_parents(Filter::Active) {
            list.push(ReviewItemMenuChoices::RemoveParent(parent.get_item()));
        }

        list.push(ReviewItemMenuChoices::AddNewChild);

        for child in current_item.get_item_node().get_children(Filter::Active) {
            let child_status = all_items
                .get(child.get_item().get_surreal_record_id())
                .expect("Child must be in the list of all items");
            list.push(ReviewItemMenuChoices::GoToChild(child_status));
        }

        for child in current_item.get_item_node().get_children(Filter::Active) {
            list.push(ReviewItemMenuChoices::RemoveChild(child.get_item()));
        }

        list
    }
}

pub(crate) async fn present_review_item_menu(
    item_status: &ItemStatus<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let item_under_review_id = item_status.get_surreal_record_id().clone();
    let mut selected_item_id = item_under_review_id.clone();
    let mut will_be_previously_selected_item_id = item_status.get_surreal_record_id().clone();

    loop {
        let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
            .await
            .unwrap();
        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let all_items = calculated_data.get_items_status();
        let base_data = calculated_data.get_base_data();

        let item_under_review = all_items
            .get(&item_under_review_id)
            .expect("Item under review must be in the list of all items");
        let previously_selected_item = all_items
            .get(&will_be_previously_selected_item_id)
            .unwrap_or(item_under_review);
        will_be_previously_selected_item_id = selected_item_id.clone();
        let selected_item = all_items
            .get(&selected_item_id)
            .expect("Selected item must be in the list of all items");

        let choices =
            ReviewItemMenuChoices::make_list(selected_item, previously_selected_item, all_items);
        let selected = Select::new("What would you like to do with this item?", choices)
            .with_page_size(default_select_page_size())
            .prompt()
            .unwrap();

        match selected {
            ReviewItemMenuChoices::DoneWithReview => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::UpdateItemLastReviewedDate(
                        item_under_review.get_surreal_record_id().clone(),
                        Utc::now().into(),
                    ))
                    .await
                    .unwrap();

                return Ok(());
            }
            ReviewItemMenuChoices::UpdateRelativeImportanceDontShowSingleParent { parent }
            | ReviewItemMenuChoices::UpdateRelativeImportanceShowParent { parent } => {
                let parent = all_items
                    .get(parent.get_surreal_record_id())
                    .expect("Parent must be in the list of all items")
                    .get_item_node();
                update_relative_importance(
                    parent,
                    selected_item.get_item(),
                    send_to_data_storage_layer,
                )
                .await
                .unwrap();
                //Go back to the parent item after updating relative importance
                selected_item_id = parent.get_surreal_record_id().clone();

                continue;
            }
            ReviewItemMenuChoices::UpdateDependencies { current_item } => {
                assert_eq!(
                    current_item.get_item(),
                    selected_item.get_item(),
                    "current_item exists twice so it can be used by display trait"
                );
                let dependencies = prompt_for_dependencies(
                    Some(selected_item),
                    base_data,
                    send_to_data_storage_layer,
                )
                .await
                .unwrap();
                for command in dependencies.into_iter() {
                    match command {
                        AddOrRemove::AddExisting(dependency) => {
                            send_to_data_storage_layer
                                .send(DataLayerCommands::AddItemDependency(
                                    selected_item.get_surreal_record_id().clone(),
                                    dependency,
                                ))
                                .await
                                .unwrap();
                        }
                        AddOrRemove::AddNewEvent(new_event) => {
                            send_to_data_storage_layer
                                .send(DataLayerCommands::AddItemDependencyNewEvent(
                                    selected_item.get_surreal_record_id().clone(),
                                    new_event,
                                ))
                                .await
                                .unwrap();
                        }
                        AddOrRemove::RemoveExisting(dependency) => {
                            send_to_data_storage_layer
                                .send(DataLayerCommands::RemoveItemDependency(
                                    selected_item.get_surreal_record_id().clone(),
                                    dependency,
                                ))
                                .await
                                .unwrap();
                        }
                    }
                }

                continue;
            }
            ReviewItemMenuChoices::UpdateReviewFrequency { .. } => {
                present_pick_item_review_frequency_menu(selected_item, send_to_data_storage_layer)
                    .await
                    .unwrap();

                continue;
            }
            ReviewItemMenuChoices::UpdateUrgencyPlan { current_item } => {
                let now = Utc::now();
                let current_urgency = current_item.get_item_node().get_urgency_plan().as_ref();
                let urgency_plan =
                    prompt_for_urgency_plan(current_urgency, &now, send_to_data_storage_layer)
                        .await;
                send_to_data_storage_layer
                    .send(DataLayerCommands::UpdateUrgencyPlan(
                        current_item.get_surreal_record_id().clone(),
                        Some(urgency_plan),
                    ))
                    .await
                    .unwrap();

                continue;
            }
            ReviewItemMenuChoices::FinishThisItem {
                previously_selected_item_id,
            } => {
                let when_finished: Datetime = (Utc::now()).into();
                send_to_data_storage_layer
                    .send(DataLayerCommands::FinishItem {
                        item: selected_item.get_surreal_record_id().clone(),
                        when_finished,
                    })
                    .await
                    .unwrap();

                if selected_item.get_item() == item_under_review.get_item() {
                    return Ok(());
                } else {
                    println!("Item finished, going back to the previously selected item:");
                    let display_item = DisplayItemStatus::new(
                        item_under_review,
                        Filter::Active,
                        DisplayFormat::SingleLine,
                    );
                    println!("{}", display_item);
                    selected_item_id = previously_selected_item_id;
                    continue;
                }
            }
            ReviewItemMenuChoices::AddNewParent => {
                give_this_item_a_parent(
                    selected_item.get_item(),
                    false,
                    send_to_data_storage_layer,
                )
                .await
                .unwrap();

                continue;
            }
            ReviewItemMenuChoices::AddNewChild => {
                state_a_smaller_action(selected_item.get_item_node(), send_to_data_storage_layer)
                    .await
                    .unwrap();

                continue;
            }
            ReviewItemMenuChoices::GoToParent(item) => {
                selected_item_id = item.get_surreal_record_id().clone();
                continue;
            }
            ReviewItemMenuChoices::RemoveParent(item) => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::ParentItemRemoveParent {
                        child: selected_item.get_surreal_record_id().clone(),
                        parent_to_remove: item.get_surreal_record_id().clone(),
                    })
                    .await
                    .unwrap();

                continue;
            }
            ReviewItemMenuChoices::GoToChild(item_status) => {
                selected_item_id = item_status.get_surreal_record_id().clone();
                continue;
            }
            ReviewItemMenuChoices::RemoveChild(item) => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::ParentItemRemoveParent {
                        child: item.get_surreal_record_id().clone(),
                        parent_to_remove: selected_item.get_surreal_record_id().clone(),
                    })
                    .await
                    .unwrap();

                continue;
            }
        }
    }
}

pub(crate) async fn update_relative_importance(
    parent: &ItemNode<'_>,
    item_to_move: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let (current_position, ..) = parent
        .get_children(Filter::Active)
        .find_position(|x| x.get_item() == item_to_move)
        .expect(
            "item to move must already be in the list so it must already be a child of the parent",
        );
    let priority_list = parent
        .get_children(Filter::Active)
        .map(|x| x.get_item())
        .filter(|x| *x != item_to_move)
        .collect::<Vec<_>>();
    let higher_than = select_higher_importance_than_this(&priority_list, Some(current_position));
    send_to_data_storage_layer
        .send(DataLayerCommands::UpdateRelativeImportance {
            parent: parent.get_surreal_record_id().clone(),
            update_this_child: item_to_move.get_surreal_record_id().clone(),
            higher_importance_than_this_child: higher_than,
        })
        .await
        .unwrap();

    Ok(())
}
