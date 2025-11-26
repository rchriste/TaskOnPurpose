pub(crate) mod change_mode;
pub(crate) mod classify_item;
pub(crate) mod do_now_list_single_item;
pub(crate) mod parent_back_to_a_motivation;
pub(crate) mod pick_item_review_frequency;
pub(crate) mod pick_what_should_be_done_first;
pub(crate) mod review_item;
pub(crate) mod search;

use std::{fmt::Display, iter::once};

use ahash::{HashMap, HashSet};
use better_term::Style;
use change_mode::present_change_mode_menu;
use chrono::{DateTime, Local, Utc};
use classify_item::present_item_needs_a_classification_menu;
use do_now_list_single_item::urgency_plan::present_set_ready_and_urgency_plan_menu;
use inquire::{InquireError, Select};
use itertools::chain;
use parent_back_to_a_motivation::present_parent_back_to_a_motivation_menu;
use pick_item_review_frequency::present_pick_item_review_frequency_menu;
use pick_what_should_be_done_first::present_pick_what_should_be_done_first_menu;
use review_item::present_review_item_menu;
use search::present_search_menu;
use crate::menu::inquire::default_select_page_size;
use surrealdb::RecordId;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, event::Event},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_item::SurrealDependency,
        surreal_tables::SurrealTables,
    },
    display::{
        display_item::DisplayItem, display_item_node::DisplayFormat,
        display_item_status::DisplayItemStatus, display_scheduled_item::DisplayScheduledItem,
        display_urgency_level_item_with_item_status::DisplayUrgencyLevelItemWithItemStatus,
    },
    menu::inquire::back_menu::present_back_menu,
    node::{
        Filter,
        action_with_item_status::ActionWithItemStatus,
        item_status::{DependencyWithItemNode, ItemStatus},
        urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
        why_in_scope_and_action_with_item_status::{WhyInScope, WhyInScopeAndActionWithItemStatus},
    },
    systems::do_now_list::{
        DoNowList,
        current_mode::{CurrentMode, SelectedSingleMode},
    },
};

use self::do_now_list_single_item::{
    present_do_now_list_item_selected, present_is_person_or_group_around_menu,
};

use super::back_menu::capture;

pub(crate) enum InquireDoNowListItem<'e> {
    CaptureNewItem,
    Search,
    ChangeMode(&'e CurrentMode),
    DeclareEvent { waiting_on: Vec<&'e Event<'e>> },
    DoNowListSingleItem(&'e UrgencyLevelItemWithItemStatus<'e>),
    RefreshList(DateTime<Local>),
    BackMenu,
    Help,
}

impl Display for InquireDoNowListItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CaptureNewItem => write!(f, "🗬   Capture New Item"),
            Self::Search => write!(f, "🔍  Search"),
            Self::DoNowListSingleItem(item) => {
                let display = DisplayUrgencyLevelItemWithItemStatus::new(
                    item,
                    Filter::Active,
                    DisplayFormat::SingleLine,
                );
                write!(f, "{}", display)
            }
            Self::ChangeMode(current_mode) => {
                let mut mode_icons = "I:".to_string();
                mode_icons.push_str(&turn_to_icons(current_mode.get_importance_in_scope()));
                mode_icons.push_str("  & ");

                mode_icons.push_str("U:");
                let urgency_mode_icons = turn_to_icons(current_mode.get_urgency_in_scope());
                mode_icons.push_str(&urgency_mode_icons);
                write!(f, "🧭  Change Mode - Currently: {}", mode_icons)
            }
            Self::RefreshList(bullet_list_created) => write!(
                f,
                "🔄  Reload List ({})",
                bullet_list_created.format("%I:%M%p")
            ),
            Self::DeclareEvent { waiting_on } => {
                if waiting_on.is_empty() {
                    write!(f, "⚡  Declare Event")
                } else if waiting_on.len() == 1 {
                    write!(
                        f,
                        "⚡  Waiting on: {}",
                        waiting_on.first().expect("len() == 1").get_summary()
                    )
                } else {
                    write!(f, "⚡  Waiting on: {} events", waiting_on.len())
                }
            }
            Self::BackMenu => write!(f, "🏠  Back Menu"),
            Self::Help => write!(f, "❓  Help"),
        }
    }
}

fn turn_to_icons(in_scope: &[SelectedSingleMode]) -> String {
    let mut mode_icons = String::default();
    if in_scope
        .iter()
        .any(|x| x == &SelectedSingleMode::AllCoreMotivationalPurposes)
    {
        mode_icons.push('🏢');
    } else {
        mode_icons.push_str("  ");
    }
    if in_scope
        .iter()
        .any(|x| x == &SelectedSingleMode::AllNonCoreMotivationalPurposes)
    {
        mode_icons.push('🏞')
    } else {
        mode_icons.push(' ')
    }

    mode_icons
}

impl<'a> InquireDoNowListItem<'a> {
    pub(crate) fn create_list(
        item_action: &'a [UrgencyLevelItemWithItemStatus<'a>],
        events: &'a HashMap<&'a RecordId, Event<'a>>,
        do_now_list_created: DateTime<Utc>,
        current_mode: &'a CurrentMode,
    ) -> Vec<InquireDoNowListItem<'a>> {
        let waiting_on = events
            .iter()
            .filter(|(_, x)| x.is_active())
            .map(|(_, x)| x)
            .collect::<Vec<_>>();
        let iter = chain!(
            once(InquireDoNowListItem::RefreshList(
                do_now_list_created.into()
            )),
            once(InquireDoNowListItem::Search),
        );
        let iter: Box<dyn Iterator<Item = InquireDoNowListItem<'a>>> = if !waiting_on.is_empty() {
            Box::new(iter.chain(once(InquireDoNowListItem::DeclareEvent { waiting_on })))
        } else {
            Box::new(iter)
        };
        chain!(
            iter,
            once(InquireDoNowListItem::ChangeMode(current_mode)),
            once(InquireDoNowListItem::CaptureNewItem),
            item_action
                .iter()
                .map(InquireDoNowListItem::DoNowListSingleItem),
            once(InquireDoNowListItem::BackMenu),
            once(InquireDoNowListItem::Help),
        )
        .collect()
    }
}

pub(crate) async fn present_normal_do_now_list_menu(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let before_db_query = Local::now();
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let elapsed = Local::now() - before_db_query;
    if elapsed > chrono::Duration::try_seconds(1).expect("valid") {
        println!("Slow to get data from database. Time taken: {}", elapsed);
    }
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let base_data_checkpoint = Utc::now();
    let calculated_data = CalculatedData::new_from_base_data(base_data);
    let calculated_data_checkpoint = Utc::now();
    let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);
    let finish_checkpoint = Utc::now();
    let elapsed = finish_checkpoint - now;
    if elapsed > chrono::Duration::try_seconds(1).expect("valid") {
        println!("Slow to create do now list. Time taken: {}", elapsed);
        println!(
            "Base data took: {}, calculated data took: {}, do now list took: {}",
            base_data_checkpoint - now,
            calculated_data_checkpoint - base_data_checkpoint,
            finish_checkpoint - calculated_data_checkpoint
        );
    }
    present_upcoming(&do_now_list);
    present_do_now_list_menu(&do_now_list, now, send_to_data_storage_layer).await
}

pub(crate) fn present_upcoming(do_now_list: &DoNowList) {
    let upcoming = do_now_list.get_upcoming();
    if !upcoming.is_empty() {
        println!("Upcoming:");
        for scheduled_item in upcoming
            .get_ordered_scheduled_items()
            .as_ref()
            .expect("upcoming is not empty")
            .iter()
            .rev()
        {
            let display_scheduled_item = DisplayScheduledItem::new(scheduled_item);
            println!("{}", display_scheduled_item);
        }
    } else if upcoming.has_conflicts() {
        let bold_text = Style::new().bold();
        let not_bold_text = Style::new();
        println!(
            "{}Scheduled items don't fit. At least one of the following items need to be adjusted:{}",
            bold_text, not_bold_text
        );
        for conflict in upcoming.get_conflicts() {
            println!("{}", DisplayItem::new(conflict));
        }
        println!();
    }
}

enum EventSelection<'e> {
    ReturnToDoNowList,
    Event(&'e Event<'e>),
}

impl Display for EventSelection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSelection::ReturnToDoNowList => write!(f, "🔙 Return to Do Now List"),
            EventSelection::Event(event) => write!(f, "{}", event.get_summary()),
        }
    }
}

enum EventTrigger<'e> {
    ReturnToDoNowList,
    TriggerEvent {
        all_items_waiting_on_event: Vec<&'e ItemStatus<'e>>,
    },
    ItemDependentOnThisEvent(&'e ItemStatus<'e>),
}

impl Display for EventTrigger<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventTrigger::ReturnToDoNowList => write!(f, "🔙 Return to Do Now List"),
            EventTrigger::TriggerEvent { .. } => {
                write!(f, "⚡ Trigger or record that this event has happened")
            }
            EventTrigger::ItemDependentOnThisEvent(item) => {
                let display =
                    DisplayItemStatus::new(item, Filter::Active, DisplayFormat::SingleLine);
                write!(f, "{}", display)
            }
        }
    }
}

pub(crate) async fn present_do_now_list_menu(
    do_now_list: &DoNowList,
    do_now_list_created: DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let ordered_do_now_list = do_now_list.get_ordered_do_now_list();
    let events = do_now_list.get_events();

    let inquire_do_now_list = InquireDoNowListItem::create_list(
        ordered_do_now_list,
        events,
        do_now_list_created,
        do_now_list.get_current_mode(),
    );

    println!();
    let starting_cursor = if ordered_do_now_list.is_empty()
        || inquire_do_now_list
            .iter()
            .any(|x| matches!(x, InquireDoNowListItem::DeclareEvent { .. }))
    {
        5
    } else {
        4
    };
    let selected = Select::new(
        "Select from this \"Do Now\" list (default choice is recommended)|",
        inquire_do_now_list,
    )
    .with_starting_cursor(starting_cursor)
    .with_page_size(default_select_page_size())
    .prompt();

    match selected {
        Ok(InquireDoNowListItem::Help) => present_do_now_help(),
        Ok(InquireDoNowListItem::CaptureNewItem) => capture(send_to_data_storage_layer).await,
        Ok(InquireDoNowListItem::Search) => {
            present_search_menu(do_now_list, send_to_data_storage_layer).await
        }
        Ok(InquireDoNowListItem::ChangeMode(current_mode)) => {
            present_change_mode_menu(current_mode, send_to_data_storage_layer).await
        }
        Ok(InquireDoNowListItem::DeclareEvent { mut waiting_on }) => {
            waiting_on.sort_by(|a, b| b.get_last_updated().cmp(a.get_last_updated()));
            let list = chain!(
                once(EventSelection::ReturnToDoNowList),
                waiting_on.into_iter().map(EventSelection::Event)
            )
            .collect::<Vec<_>>();
            let selected = Select::new("Select the event that just happened|", list)
                .with_page_size(default_select_page_size())
                .prompt();
            match selected {
                Ok(EventSelection::Event(event)) => {
                    let items_waiting_on_this_event = do_now_list
                        .get_all_items_status()
                        .values()
                        .filter(|x| {
                            x.is_active()
                                && x.get_dependencies(Filter::Active).any(|x| match x {
                                    DependencyWithItemNode::AfterEvent(event_waiting_on) => {
                                        event_waiting_on.get_surreal_record_id()
                                            == event.get_surreal_record_id()
                                    }
                                    _ => false,
                                })
                        })
                        .collect::<Vec<_>>();
                    let list = chain!(
                        once(EventTrigger::ReturnToDoNowList),
                        once(EventTrigger::TriggerEvent {
                            all_items_waiting_on_event: items_waiting_on_this_event.clone()
                        }),
                        items_waiting_on_this_event
                            .iter()
                            .copied()
                            .map(EventTrigger::ItemDependentOnThisEvent)
                    )
                    .collect::<Vec<_>>();
                    let selected = Select::new(
                        "Clear event or select an item that is dependent on this event|",
                        list,
                    )
                    .with_page_size(default_select_page_size())
                    .prompt();
                    match selected {
                        Ok(EventTrigger::TriggerEvent {
                            all_items_waiting_on_event,
                        }) => {
                            //Clear the event before clearing the trigger in case it is cancelled part way through
                            for item_waiting_on_event in all_items_waiting_on_event {
                                send_to_data_storage_layer
                                    .send(DataLayerCommands::RemoveItemDependency(
                                        item_waiting_on_event.get_surreal_record_id().clone(),
                                        SurrealDependency::AfterEvent(
                                            event.get_surreal_record_id().clone(),
                                        ),
                                    ))
                                    .await
                                    .unwrap();
                            }
                            send_to_data_storage_layer
                                .send(DataLayerCommands::TriggerEvent {
                                    event: event.get_surreal_record_id().clone(),
                                    when: Utc::now().into(),
                                })
                                .await
                                .unwrap();
                            Ok(())
                        }
                        Ok(EventTrigger::ItemDependentOnThisEvent(item_status)) => {
                            let mut why_in_scope = HashSet::default();
                            why_in_scope.insert(WhyInScope::MenuNavigation);
                            Box::pin(present_do_now_list_item_selected(
                                item_status,
                                &why_in_scope,
                                Utc::now(),
                                do_now_list,
                                send_to_data_storage_layer,
                            ))
                            .await
                        }
                        Ok(EventTrigger::ReturnToDoNowList)
                        | Err(InquireError::OperationCanceled) => Ok(()),
                        Err(InquireError::OperationInterrupted) => Err(()),
                        Err(err) => {
                            panic!("Unexpected error, try restarting the terminal: {}", err)
                        }
                    }
                }
                Ok(EventSelection::ReturnToDoNowList) | Err(InquireError::OperationCanceled) => {
                    Ok(())
                }
                Err(InquireError::OperationInterrupted) => Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }
        Ok(InquireDoNowListItem::DoNowListSingleItem(selected)) => match selected {
            UrgencyLevelItemWithItemStatus::MultipleItems(choices) => {
                present_pick_what_should_be_done_first_menu(
                    choices,
                    do_now_list,
                    send_to_data_storage_layer,
                )
                .await
            }
            UrgencyLevelItemWithItemStatus::SingleItem(
                why_in_scope_and_action_with_item_status,
            ) => {
                let why_in_scope = why_in_scope_and_action_with_item_status.get_why_in_scope();
                match why_in_scope_and_action_with_item_status.get_action() {
                    ActionWithItemStatus::PickItemReviewFrequency(item_status) => {
                        present_pick_item_review_frequency_menu(
                            item_status,
                            send_to_data_storage_layer,
                        )
                        .await
                    }
                    ActionWithItemStatus::ItemNeedsAClassification(item_status) => {
                        present_item_needs_a_classification_menu(
                            item_status,
                            send_to_data_storage_layer,
                        )
                        .await
                    }
                    ActionWithItemStatus::ReviewItem(item_status) => {
                        let base_data = do_now_list.get_base_data();
                        present_review_item_menu(
                            item_status,
                            do_now_list.get_all_items_status(),
                            base_data,
                            send_to_data_storage_layer,
                        )
                        .await
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
                                Utc::now(),
                                do_now_list,
                                send_to_data_storage_layer,
                            ))
                            .await
                        }
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
                        present_parent_back_to_a_motivation_menu(
                            item_status,
                            send_to_data_storage_layer,
                        )
                        .await
                    }
                }
            }
        },
        Ok(InquireDoNowListItem::RefreshList(..)) | Err(InquireError::OperationCanceled) => {
            println!("Press Ctrl+C to exit");
            Ok(())
        }
        Ok(InquireDoNowListItem::BackMenu) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

enum DoNowHelpChoices {
    GettingStarted,
    HowWorkIsScheduled,
    Workarounds,
    ReturnToDoNowList,
}

impl Display for DoNowHelpChoices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoNowHelpChoices::GettingStarted => write!(f, "How to Get Started"),
            DoNowHelpChoices::HowWorkIsScheduled => write!(f, "How Work is Scheduled"),
            DoNowHelpChoices::Workarounds => {
                write!(f, "Workarounds for features not yet implemented")
            }
            DoNowHelpChoices::ReturnToDoNowList => write!(f, "🔙 Return to Do Now List"),
        }
    }
}

pub(crate) fn present_do_now_help() -> Result<(), ()> {
    let choices = vec![
        DoNowHelpChoices::GettingStarted,
        DoNowHelpChoices::HowWorkIsScheduled,
        DoNowHelpChoices::Workarounds,
        DoNowHelpChoices::ReturnToDoNowList,
    ];
    let selected = Select::new("Select from the below list|", choices)
        .with_page_size(default_select_page_size())
        .prompt();

    match selected {
        Ok(DoNowHelpChoices::GettingStarted) => {
            present_do_now_help_getting_started()?;
            present_do_now_help()
        }
        Ok(DoNowHelpChoices::HowWorkIsScheduled) => {
            present_do_now_how_work_is_scheduled()?;
            present_do_now_help()
        }
        Ok(DoNowHelpChoices::Workarounds) => {
            present_do_now_help_workarounds()?;
            present_do_now_help()
        }
        Ok(DoNowHelpChoices::ReturnToDoNowList) | Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) fn present_do_now_help_getting_started() -> Result<(), ()> {
    println!();
    println!("Getting Started Help Coming Soon!");
    println!();
    Ok(())
}

pub(crate) fn present_do_now_how_work_is_scheduled() -> Result<(), ()> {
    println!();
    println!("How Work is Scheduled Help Coming Soon!");
    println!();
    Ok(())
}

pub(crate) fn present_do_now_help_workarounds() -> Result<(), ()> {
    println!();
    println!("Workarounds Help Coming Soon!");
    println!();
    Ok(())
}
