use std::fmt::{self, Display, Formatter};

use ahash::HashSet;
use chrono::Utc;
use inquire::{InquireError, MultiSelect, Select};
use rand::Rng;
use tokio::sync::mpsc::Sender;

use crate::data_storage::surrealdb_layer::SurrealTrigger;

use crate::{
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_in_the_moment_priority::SurrealPriorityKind,
    },
    display::{
        display_item_node::DisplayFormat,
        display_why_in_scope_and_action_with_item_status::DisplayWhyInScopeAndActionWithItemStatus,
    },
    menu::inquire::{
        default_select_page_size,
        do_now_list_menu::do_now_list_single_item::urgency_plan::prompt_for_triggers,
    },
    node::Filter,
    systems::do_now_list::DoNowList,
};

use super::WhyInScopeAndActionWithItemStatus;

enum PriorityWizardMode {
    PriorityWizard,
    Legacy,
}

impl Display for PriorityWizardMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PriorityWizardMode::PriorityWizard => write!(f, "Priority Wizard"),
            PriorityWizardMode::Legacy => write!(f, "Legacy Mode (Current Behavior)"),
        }
    }
}

enum FinalPriorityWizardChoice {
    PickRandom,
    RepeatProcess,
}

impl Display for FinalPriorityWizardChoice {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FinalPriorityWizardChoice::PickRandom => write!(f, "Pick one at random"),
            FinalPriorityWizardChoice::RepeatProcess => write!(f, "Repeat the process"),
        }
    }
}

enum NoSelectionChoice {
    SelectAnItem,
    SkipToNext,
}

impl Display for NoSelectionChoice {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NoSelectionChoice::SelectAnItem => write!(f, "Select an item to work on"),
            NoSelectionChoice::SkipToNext => write!(f, "Skip to next comparison"),
        }
    }
}

pub(crate) async fn present_priority_wizard_or_legacy<'a>(
    choices: &'a [WhyInScopeAndActionWithItemStatus<'a>],
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let mode_selection = Select::new(
        "How would you like to select an item?",
        vec![
            PriorityWizardMode::PriorityWizard,
            PriorityWizardMode::Legacy,
        ],
    )
    .with_page_size(default_select_page_size())
    .prompt();

    match mode_selection {
        Ok(PriorityWizardMode::PriorityWizard) => {
            priority_wizard_loop(choices, do_now_list, send_to_data_storage_layer).await
        }
        Ok(PriorityWizardMode::Legacy) => {
            super::present_pick_what_should_be_done_first_menu(
                choices,
                do_now_list,
                send_to_data_storage_layer,
            )
            .await
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

async fn priority_wizard_loop<'a>(
    choices: &'a [WhyInScopeAndActionWithItemStatus<'a>],
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    // Track items that have been selected already
    let mut selected_items = HashSet::default();
    let mut used_as_highest_priority = HashSet::default();
    let mut rng = rand::rng();

    loop {
        // Find items that haven't been selected yet
        let unselected_items: Vec<_> = choices
            .iter()
            .filter(|item| !selected_items.contains(item.get_surreal_record_id()))
            .collect();

        if unselected_items.len() <= 1 {
            // All items have been selected - break to final choice
            break Ok(());
        }

        let items_still_to_use: Vec<_> = unselected_items
            .iter()
            .filter(|item| !used_as_highest_priority.contains(item.get_surreal_record_id()))
            .collect();

        // Check if all items have been compared
        if items_still_to_use.is_empty() {
            let final_choice = Select::new(
                "All items have been compared. What would you like to do?",
                vec![
                    FinalPriorityWizardChoice::PickRandom,
                    FinalPriorityWizardChoice::RepeatProcess,
                ],
            )
            .with_page_size(default_select_page_size())
            .prompt();

            match final_choice {
                Ok(FinalPriorityWizardChoice::PickRandom) => {
                    // Pick a random item from all choices and set it as higher priority for 1 minute
                    let random_idx = rng.random_range(0..unselected_items.len());
                    let random_choice = &unselected_items[random_idx];

                    // Set this item as higher priority than all others for 1 minute
                    let now = Utc::now();
                    let one_minute_trigger = vec![SurrealTrigger::WallClockDateTime(
                        (now + chrono::Duration::try_minutes(1).expect("valid")).into(),
                    )];

                    let other_choices: Vec<_> = unselected_items
                        .iter()
                        .filter(|item| {
                            item.get_surreal_record_id() != random_choice.get_surreal_record_id()
                        })
                        .map(|item| item.clone_to_surreal_action())
                        .collect();

                    if !other_choices.is_empty() {
                        send_to_data_storage_layer
                            .send(DataLayerCommands::DeclareInTheMomentPriority {
                                choice: random_choice.clone_to_surreal_action(),
                                kind: SurrealPriorityKind::HighestPriority,
                                not_chosen: other_choices,
                                in_effect_until: one_minute_trigger,
                            })
                            .await
                            .unwrap();
                    }

                    // Return Ok to exit and let main loop refresh data from database
                    return Ok(());
                }
                Ok(FinalPriorityWizardChoice::RepeatProcess) => {
                    used_as_highest_priority.clear();
                    continue;
                }
                Err(InquireError::OperationCanceled) => {
                    return Ok(());
                }
                Err(InquireError::OperationInterrupted) => return Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }

        // Pick a random unselected item
        let random_idx = rng.random_range(0..items_still_to_use.len());
        let selected_at_random = items_still_to_use[random_idx];
        used_as_highest_priority.insert(selected_at_random.get_surreal_record_id());

        // Show the selected item in multiline format with hierarchy reversed
        let display_selected = DisplayWhyInScopeAndActionWithItemStatus::new(
            selected_at_random,
            Filter::Active,
            DisplayFormat::MultiLineTreeReversed,
        );

        // Get all other unselected items for comparison
        let comparison_choices: Vec<_> = unselected_items
            .iter()
            .filter(|item| {
                item.get_surreal_record_id() != selected_at_random.get_surreal_record_id()
            })
            .map(|item| {
                DisplayWhyInScopeAndActionWithItemStatus::new(
                    item,
                    Filter::Active,
                    DisplayFormat::SingleLine,
                )
            })
            .collect();

        println!("\nComparing item:\n{}\n", display_selected);

        // Ask which items this is higher priority than

        let selected_from = MultiSelect::new(
            "Which items is this HIGHER priority than? (Press Enter to skip to next item)",
            comparison_choices.clone(),
        )
        .with_page_size(default_select_page_size())
        .prompt();

        let higher_priority_than = match selected_from {
            Ok(selected) => selected,
            Err(InquireError::OperationCanceled) => {
                return Ok(());
            }
            Err(InquireError::OperationInterrupted) => return Err(()),
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        };

        // If user selected any items, ask for duration and save priorities
        if !higher_priority_than.is_empty() {
            println!("How long should this priority comparison be in effect?");
            let now = Utc::now();
            let in_effect_until = prompt_for_triggers(&now, send_to_data_storage_layer).await;

            // Send a message for each item that this is higher priority than
            // and mark those items as "selected" (eliminated from future consideration)
            for lower_priority_item in &higher_priority_than {
                send_to_data_storage_layer
                    .send(DataLayerCommands::DeclareInTheMomentPriority {
                        choice: selected_at_random.clone_to_surreal_action(),
                        kind: SurrealPriorityKind::HighestPriority,
                        not_chosen: vec![lower_priority_item.clone_to_surreal_action()],
                        in_effect_until: in_effect_until.clone(),
                    })
                    .await
                    .unwrap();

                selected_items.insert(lower_priority_item.get_surreal_record_id().clone());
            }
        } else {
            // User didn't select any items - ask what they want to do
            let no_selection_choice = Select::new(
                "You didn't select any items. What would you like to do?",
                vec![
                    NoSelectionChoice::SelectAnItem,
                    NoSelectionChoice::SkipToNext,
                ],
            )
            .with_page_size(default_select_page_size())
            .prompt();

            match no_selection_choice {
                Ok(NoSelectionChoice::SelectAnItem) => {
                    // Show all unselected items including the current one for selection
                    let all_items_for_selection: Vec<_> = unselected_items
                        .iter()
                        .map(|item| {
                            DisplayWhyInScopeAndActionWithItemStatus::new(
                                item,
                                Filter::Active,
                                DisplayFormat::SingleLine,
                            )
                        })
                        .collect();

                    let item_selection = Select::new(
                        "Which item would you like to work on?",
                        all_items_for_selection,
                    )
                    .with_page_size(default_select_page_size())
                    .prompt();

                    match item_selection {
                        Ok(selected_display) => {
                            // Route to the appropriate menu based on action type
                            super::handle_item_selection(
                                selected_display.into(),
                                do_now_list,
                                send_to_data_storage_layer,
                            )
                            .await?;

                            // After returning from the menu, return Ok to refresh the main loop
                            return Ok(());
                        }
                        Err(InquireError::OperationCanceled) => {
                            // User canceled - continue to next comparison
                        }
                        Err(InquireError::OperationInterrupted) => return Err(()),
                        Err(err) => {
                            panic!("Unexpected error, try restarting the terminal: {}", err)
                        }
                    }
                }
                Ok(NoSelectionChoice::SkipToNext) => {
                    // Continue to next comparison - do nothing
                }
                Err(InquireError::OperationCanceled) => {
                    return Ok(());
                }
                Err(InquireError::OperationInterrupted) => return Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }

        // Continue the loop with the next random item
    }
}
