pub(crate) mod priority_wizard;

use std::fmt::{self, Display, Formatter};

use chrono::Utc;
use inquire::{InquireError, MultiSelect, Select};
use rand::Rng;
use tokio::sync::mpsc::Sender;

use crate::{
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_in_the_moment_priority::SurrealPriorityKind,
    },
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

use super::{
    WhyInScopeAndActionWithItemStatus, do_now_list_single_item::urgency_plan::prompt_for_triggers,
};

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

enum HighestOrLowest {
    PickThisTime,
    RecordHighestPriorityUntil,
    RecordLowestPriorityUntil,
    FinishOrRetireItem,
}

impl Display for HighestOrLowest {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            HighestOrLowest::PickThisTime => write!(f, "Pick This Once"),
            HighestOrLowest::RecordHighestPriorityUntil => {
                write!(f, "Set as a higher priority...")
            }
            HighestOrLowest::RecordLowestPriorityUntil => {
                write!(f, "Set as a lower priority...")
            }
            HighestOrLowest::FinishOrRetireItem => {
                write!(f, "Finish or retire item")
            }
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

    let highest_or_lowest = match Select::new(
        "Highest or lowest priority?",
        vec![
            HighestOrLowest::PickThisTime,
            HighestOrLowest::RecordHighestPriorityUntil,
            HighestOrLowest::RecordLowestPriorityUntil,
            HighestOrLowest::FinishOrRetireItem,
        ],
    )
    .with_page_size(default_select_page_size())
    .prompt()
    {
        Ok(highest_or_lowest) => highest_or_lowest,
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

    let highest_or_lowest = match highest_or_lowest {
        HighestOrLowest::RecordHighestPriorityUntil => SurrealPriorityKind::HighestPriority,
        HighestOrLowest::RecordLowestPriorityUntil => SurrealPriorityKind::LowestPriority,
        HighestOrLowest::FinishOrRetireItem => {
            let now = Utc::now();
            send_to_data_storage_layer
                .send(DataLayerCommands::FinishItem {
                    item: choice.get_surreal_record_id().clone(),
                    when_finished: now.into(),
                })
                .await
                .unwrap();

            return Ok(());
        }
        HighestOrLowest::PickThisTime => {
            // Get the inner WhyInScopeAndActionWithItemStatus from the display wrapper
            let original_choice = choice.into();

            return handle_item_selection(original_choice, do_now_list, send_to_data_storage_layer)
                .await;
        }
    };

    let other_items = choices
        .iter()
        .filter(|x| x.get_action() != choice.get_action())
        .map(|x| {
            DisplayWhyInScopeAndActionWithItemStatus::new(
                x,
                Filter::Active,
                DisplayFormat::SingleLine,
            )
        })
        .collect::<Vec<_>>();

    //Set the starting position to the choice so the other options you see are the same ones that you just saw in the previous menu
    let starting_position = choices
        .iter()
        .enumerate()
        .find_map(|(index, x)| {
            if x.get_action() == choice.get_action() {
                Some(index % (choices.len() - 1)) //The -1 is to account for the fact that the choice is not in the list of other items, we need to mod (%) to make sure we don't go out of bounds because of this
            } else {
                None
            }
        })
        .expect("Choice just selected will always be found");
    let other_items = MultiSelect::new("What other items should be affected?", other_items)
        .with_page_size(default_select_page_size())
        .with_starting_cursor(starting_position)
        .prompt();

    let not_chosen = match other_items {
        Ok(not_chosen) => not_chosen
            .into_iter()
            .map(|x| x.clone_to_surreal_action())
            .collect(),
        Err(InquireError::OperationCanceled) => {
            return Ok(()); // Do nothing, just continue
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    println!("How long should this be in effect?");
    let now = Utc::now();
    let in_effect_until = prompt_for_triggers(None, &now, send_to_data_storage_layer).await;

    send_to_data_storage_layer
        .send(DataLayerCommands::DeclareInTheMomentPriority {
            choice: choice.clone_to_surreal_action(),
            kind: highest_or_lowest,
            not_chosen,
            in_effect_until,
        })
        .await
        .unwrap();

    Ok(())
}
