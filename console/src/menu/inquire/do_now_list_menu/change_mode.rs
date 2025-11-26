use std::fmt::{Display, Formatter};

use inquire::InquireError;
use tokio::sync::mpsc::Sender;

use crate::{
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands,
        surreal_current_mode::{NewCurrentMode, SurrealSelectedSingleMode},
    },
    menu::inquire::{default_select_page_size, do_now_list_menu::present_normal_do_now_list_menu},
    systems::do_now_list::current_mode::{CurrentMode, SelectedSingleMode},
};

#[derive(PartialEq, Eq)]
pub(crate) enum InTheModeChoices {
    CoreWork,
    NonCoreWork,
}

impl Display for InTheModeChoices {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CoreWork => write!(f, "🏢 Core Work"),
            Self::NonCoreWork => write!(f, "🏞 Non-Core Work"),
        }
    }
}

pub(crate) async fn present_change_mode_menu(
    current_mode: &CurrentMode,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let choices = vec![InTheModeChoices::CoreWork, InTheModeChoices::NonCoreWork];
    let mut default = Vec::default();
    if current_mode
        .get_importance_in_scope()
        .iter()
        .any(|x| x == &SelectedSingleMode::AllCoreMotivationalPurposes)
    {
        default.push(0);
    }
    if current_mode
        .get_importance_in_scope()
        .iter()
        .any(|x| x == &SelectedSingleMode::AllNonCoreMotivationalPurposes)
    {
        default.push(1);
    }
    let selections =
        inquire::MultiSelect::new("What Motivational Purposes to show for Importance", choices)
            .with_page_size(default_select_page_size())
            .with_default(&default)
            .prompt();
    let importance_choice = match selections {
        Ok(selections) => {
            let mut choices: Vec<SurrealSelectedSingleMode> = Vec::default();
            if selections.contains(&InTheModeChoices::CoreWork) {
                choices.push(SurrealSelectedSingleMode::AllCoreMotivationalPurposes);
            }
            if selections.contains(&InTheModeChoices::NonCoreWork) {
                choices.push(SurrealSelectedSingleMode::AllNonCoreMotivationalPurposes);
            }
            choices
        }
        Err(InquireError::OperationCanceled) => {
            return Box::pin(present_normal_do_now_list_menu(send_to_data_storage_layer)).await;
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    let choices = vec![InTheModeChoices::CoreWork, InTheModeChoices::NonCoreWork];

    let mut default = Vec::default();
    if current_mode
        .get_urgency_in_scope()
        .iter()
        .any(|x| x == &SelectedSingleMode::AllCoreMotivationalPurposes)
    {
        default.push(0);
    }
    if current_mode
        .get_urgency_in_scope()
        .iter()
        .any(|x| x == &SelectedSingleMode::AllNonCoreMotivationalPurposes)
    {
        default.push(1);
    }

    let selections =
        inquire::MultiSelect::new("What Motivational Purposes to show for Urgency", choices)
            .with_page_size(default_select_page_size())
            .with_default(&default)
            .prompt();
    let urgency_choice = match selections {
        Ok(selections) => {
            let mut choices: Vec<SurrealSelectedSingleMode> = Vec::default();
            if selections.contains(&InTheModeChoices::CoreWork) {
                choices.push(SurrealSelectedSingleMode::AllCoreMotivationalPurposes);
            }
            if selections.contains(&InTheModeChoices::NonCoreWork) {
                choices.push(SurrealSelectedSingleMode::AllNonCoreMotivationalPurposes);
            }
            choices
        }
        Err(InquireError::OperationCanceled) => {
            return Box::pin(present_normal_do_now_list_menu(send_to_data_storage_layer)).await;
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    if urgency_choice.is_empty() && importance_choice.is_empty() {
        println!("Something must be selected. Try again.");
        return Box::pin(present_change_mode_menu(
            current_mode,
            send_to_data_storage_layer,
        ))
        .await;
    };

    let new_current_mode = NewCurrentMode::new(urgency_choice, importance_choice);
    send_to_data_storage_layer
        .send(DataLayerCommands::SetCurrentMode(new_current_mode))
        .await
        .unwrap();

    Ok(())
}
