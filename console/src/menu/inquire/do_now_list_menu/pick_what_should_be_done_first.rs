pub(crate) mod priority_wizard;

use inquire::{InquireError, Select};
use rand::Rng;
use tokio::sync::mpsc::Sender;

use crate::{
    data_storage::surrealdb_layer::data_layer_commands::DataLayerCommands,
    display::{
        display_item_node::DisplayFormat,
        display_why_in_scope_and_action_with_item_status::DisplayWhyInScopeAndActionWithItemStatus,
    },
    menu::inquire::do_now_list_menu::{
        classify_item::present_item_needs_a_classification_menu,
        do_now_list_single_item::{
            present_do_now_list_item_selected, present_is_person_or_group_around_menu,
            urgency_plan::present_set_ready_and_urgency_plan_menu,
        },
        parent_back_to_a_motivation::present_parent_back_to_a_motivation_menu,
        pick_item_review_frequency::present_pick_item_review_frequency_menu,
        present_do_now_list_menu,
        review_item::present_review_item_menu,
    },
    node::{Filter, action_with_item_status::ActionWithItemStatus},
    systems::do_now_list::DoNowList,
};

use crate::menu::inquire::default_select_page_size;

use super::WhyInScopeAndActionWithItemStatus;

/// Routes a selected item to the appropriate menu based on its action type
pub(super) async fn handle_item_selection<'a>(
    selected_item: &'a WhyInScopeAndActionWithItemStatus<'a>,
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let why_in_scope = selected_item.get_why_in_scope();
    let item_action = selected_item.get_action();

    match item_action {
        ActionWithItemStatus::PickItemReviewFrequency(item_status) => {
            present_pick_item_review_frequency_menu(item_status, send_to_data_storage_layer).await
        }
        ActionWithItemStatus::ReviewItem(item_status) => {
            present_review_item_menu(item_status, send_to_data_storage_layer).await
        }
        ActionWithItemStatus::MakeProgress(item_status) => {
            if item_status.is_person_or_group() {
                present_is_person_or_group_around_menu(
                    item_status.get_item_node(),
                    send_to_data_storage_layer,
                )
                .await
            } else {
                Box::pin(present_do_now_list_item_selected(
                    item_status,
                    why_in_scope,
                    chrono::Utc::now(),
                    do_now_list,
                    send_to_data_storage_layer,
                ))
                .await
            }
        }
        ActionWithItemStatus::ItemNeedsAClassification(item_status) => {
            present_item_needs_a_classification_menu(item_status, send_to_data_storage_layer).await
        }
        ActionWithItemStatus::SetReadyAndUrgency(item_status) => {
            let base_data = do_now_list.get_base_data();
            present_set_ready_and_urgency_plan_menu(
                item_status,
                base_data,
                send_to_data_storage_layer,
            )
            .await
        }
        ActionWithItemStatus::ParentBackToAMotivation(item_status) => {
            present_parent_back_to_a_motivation_menu(item_status, send_to_data_storage_layer).await
        }
    }
}

pub(crate) async fn present_pick_what_should_be_done_first_menu<'a>(
    choices: &'a [WhyInScopeAndActionWithItemStatus<'a>],
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let display_choices = choices
        .iter()
        .map(|x| {
            DisplayWhyInScopeAndActionWithItemStatus::new(
                x,
                Filter::Active,
                DisplayFormat::SingleLine,
            )
        })
        .collect::<Vec<_>>();

    let starting_choice = rand::rng().random_range(0..display_choices.len());
    let choice = Select::new("Pick a priority?", display_choices)
        .with_page_size(default_select_page_size())
        .with_starting_cursor(starting_choice)
        .prompt();
    let choice = match choice {
        Ok(choice) => choice,
        Err(InquireError::OperationCanceled) => {
            return Box::pin(present_do_now_list_menu(
                do_now_list,
                *do_now_list.get_now(),
                send_to_data_storage_layer,
            ))
            .await;
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    // Removing the intermediate submenu means selecting an item directly performs the
    // same behavior as the prior "Pick This Once" choice.
    let original_choice = choice.into();
    handle_item_selection(original_choice, do_now_list, send_to_data_storage_layer).await
}
