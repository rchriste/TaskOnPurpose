use std::{fmt::Display, time::Duration};

use ahash::HashSet;
use chrono::{DateTime, Local, Utc};
use inquire::{InquireError, Select, Text};
use surrealdb::sql::Datetime;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_in_the_moment_priority::SurrealAction,
        surreal_time_spent::SurrealDedication,
    },
    display::display_duration::DisplayDuration,
    menu::inquire::{
        parse_exact_or_relative_datetime, parse_exact_or_relative_datetime_help_string,
    },
    new_time_spent::NewTimeSpent,
    node::{Filter, item_status::ItemStatus, why_in_scope_and_action_with_item_status::ToSurreal},
};

use super::WhyInScope;

pub(crate) async fn log_worked_on_this(
    selected: &ItemStatus<'_>,
    why_in_scope: &HashSet<WhyInScope>,
    when_selected: &DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    //TODO: Change Error return type to a custom struct ExitProgram or something
    // This logs time spent on an item with the goal of in the future making it possible for the user to adjust items and balance
    // Logs the following:
    // When starting to work on item
    // -urgency in list
    let urgency = selected.get_urgency_now().cloned();

    let working_on = create_working_on_list(selected);
    // -When started
    loop {
        let (when_started, when_stopped) = ask_when_started_and_stopped(
            send_to_data_storage_layer,
            when_selected,
            selected.get_now(),
        )
        .await?;
        // -When marked "I worked on this"
        // -How much time spent, show amount of time since started and show amount of time since last item completed, or allow user to enter a duration
        if let Some(dedication) = ask_about_dedication()? {
            let time_spent = NewTimeSpent {
                why_in_scope: why_in_scope.to_surreal(),
                working_on,
                urgency: urgency.unwrap_or_default(),
                when_started,
                when_stopped,
                dedication: Some(dedication),
            };
            send_to_data_storage_layer
                .send(DataLayerCommands::RecordTimeSpent(time_spent))
                .await
                .unwrap();
            break;
        } else {
            continue; //If the user cancels they should be able to try again
        }
    }
    Ok(())
}

fn create_working_on_list(selected: &ItemStatus<'_>) -> Vec<SurrealAction> {
    selected
        .get_self_and_parents_flattened(Filter::Active)
        .iter()
        .map(|x| SurrealAction::MakeProgress(x.get_surreal_record_id().clone()))
        .collect()
}

#[derive(Clone)]
enum StartedWhen {
    WhenLastItemFinished(DateTime<Local>),
    WhenBulletListWasFirstShown(DateTime<Local>),
    WhenThisItemWasSelected(DateTime<Local>),
    CustomTime,
}

impl Display for StartedWhen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartedWhen::WhenThisItemWasSelected(when_selected) => {
                write!(
                    f,
                    "When this item was selected (i.e. {})",
                    when_selected.format("%a %d %b %Y %I:%M:%S%p")
                )
            }
            StartedWhen::WhenLastItemFinished(when_last_item_finished) => {
                write!(
                    f,
                    "When the last item finished (i.e. {})",
                    when_last_item_finished.format("%a %d %b %Y %I:%M:%S%p")
                )
            }
            StartedWhen::WhenBulletListWasFirstShown(when_bullet_list_was_first_shown) => {
                write!(
                    f,
                    "When the bullet list was first shown (i.e. {})",
                    when_bullet_list_was_first_shown.format("%a %d %b %Y %I:%M:%S%p")
                )
            }
            StartedWhen::CustomTime => write!(f, "Enter a Time"),
        }
    }
}

enum StoppedWhen {
    Now,
    ManualTime,
}

impl Display for StoppedWhen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoppedWhen::Now => write!(f, "Now"),
            StoppedWhen::ManualTime => write!(f, "Enter a Time"),
        }
    }
}

enum YesOrNo {
    Yes,
    No,
}

impl Display for YesOrNo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YesOrNo::Yes => write!(f, "Yes"),
            YesOrNo::No => write!(f, "No"),
        }
    }
}

async fn ask_when_started_and_stopped(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
    when_selected: &DateTime<Utc>,
    bullet_list_created: &DateTime<Utc>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), ()> {
    let when_last_time_finished = get_when_the_last_item_finished(send_to_data_storage_layer).await;

    let mut started_when = Vec::default();

    if let Some(when_last_time_finished) = when_last_time_finished {
        started_when.push(StartedWhen::WhenLastItemFinished(
            when_last_time_finished.into(),
        ));
    }

    started_when.push(StartedWhen::WhenBulletListWasFirstShown(
        (*bullet_list_created).into(),
    ));
    started_when.push(StartedWhen::WhenThisItemWasSelected(
        (*when_selected).into(),
    ));
    started_when.push(StartedWhen::CustomTime);

    'outer: loop {
        let started_when = Select::new(
            "When did you start working on this item?",
            started_when.clone(),
        )
        .prompt();
        let when_started = match started_when {
            Ok(StartedWhen::WhenLastItemFinished(when_last_time_finished)) => {
                when_last_time_finished
            }
            Ok(StartedWhen::WhenThisItemWasSelected(when_selected)) => when_selected,
            Ok(StartedWhen::WhenBulletListWasFirstShown(when_bullet_list_was_first_shown)) => {
                when_bullet_list_was_first_shown
            }
            Ok(StartedWhen::CustomTime) => loop {
                match Text::new("Enter relative or the exact time when this item was started. Use the word \"ago\" like \"30m ago\" (\"?\" for help).\n|")
                    .prompt()
                {
                    Ok(when_started) => match parse_exact_or_relative_datetime(&when_started) {
                        Some(when_started) => break when_started,
                        None => {
                            println!("Invalid input. Please try again.");
                            println!();
                            println!("{}", parse_exact_or_relative_datetime_help_string());
                            continue;
                        }
                    },
                    Err(InquireError::OperationCanceled) => continue 'outer,
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
                }
            },
            Err(InquireError::OperationCanceled) => {
                todo!("Operation Canceled")
            }
            Err(InquireError::OperationInterrupted) => {
                return Err(());
            }
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        };
        println!("When started: {:?}", when_started);
        let stopped_when = vec![StoppedWhen::Now, StoppedWhen::ManualTime];
        let stopped_when =
            Select::new("When did you stop working on this item?", stopped_when).prompt();
        let when_stopped = match stopped_when {
            Ok(StoppedWhen::Now) => Local::now(),
            Ok(StoppedWhen::ManualTime) => loop {
                match Text::new("Enter relative or the exact time when this item was stopped. Use the word \"ago\" like \"30m ago\" (\"?\" for help).\n|")
                    .prompt()
                {
                    Ok(when_stopped) => match parse_exact_or_relative_datetime(&when_stopped) {
                        Some(when_stopped) => break when_stopped,
                        None => {
                            println!("Invalid input. Please try again.");
                            println!();
                            println!("{}", parse_exact_or_relative_datetime_help_string());
                            continue;
                        }
                    },
                    Err(InquireError::OperationCanceled) => continue 'outer,
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
                }
            },
            Err(InquireError::OperationCanceled) => continue,
            Err(InquireError::OperationInterrupted) => return Err(()),
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        };

        if when_started > when_stopped {
            println!("The time started is after the time stopped. Please try again.");
            continue;
        }
        let time_spent = when_stopped - when_started;
        let duration: Duration = Duration::from_secs(time_spent.num_seconds() as u64);
        let display_duration = DisplayDuration::new(&duration);
        println!("Time spent: {}", display_duration);
        if time_spent.num_hours() > 2 {
            let confirm = Select::new(
                &format!(
                    "The amount of time spent is {display_duration}. Are you sure this is correct?"
                ),
                vec![YesOrNo::Yes, YesOrNo::No],
            )
            .prompt();
            match confirm {
                Ok(YesOrNo::Yes) => {}
                Ok(YesOrNo::No) => continue,
                Err(InquireError::OperationCanceled) => continue,
                Err(InquireError::OperationInterrupted) => return Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }

        return Ok((when_started.into(), when_stopped.into()));
    }
}

async fn get_when_the_last_item_finished(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Option<DateTime<Utc>> {
    let (sender, receiver) = oneshot::channel();
    send_to_data_storage_layer
        .send(DataLayerCommands::SendTimeSpentLog(sender))
        .await
        .unwrap();
    let time_spent_log = receiver.await.unwrap();
    let mut earliest: Option<Datetime> = None;
    for entry in time_spent_log {
        if let Some(e) = &earliest {
            if *e < entry.when_stopped {
                earliest = Some(entry.when_stopped);
            }
        } else {
            earliest = Some(entry.when_stopped);
        }
    }

    earliest.map(|earliest| earliest.into())
}

enum Dedication {
    Primary,
    Background,
}

impl Display for Dedication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dedication::Primary => write!(f, "Primary or Main Task"),
            Dedication::Background => {
                write!(f, "Background Task, with another Task in the Foreground")
            }
        }
    }
}

fn ask_about_dedication() -> Result<Option<SurrealDedication>, ()> {
    let dedication = vec![Dedication::Primary, Dedication::Background];
    let dedication = Select::new("What is the dedication of this time spent?", dedication).prompt();
    match dedication {
        Ok(Dedication::Primary) => Ok(Some(SurrealDedication::PrimaryTask)),
        Ok(Dedication::Background) => Ok(Some(SurrealDedication::BackgroundTask)),
        Err(InquireError::OperationCanceled) => Ok(None),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}
