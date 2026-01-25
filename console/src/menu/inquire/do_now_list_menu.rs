pub(crate) mod change_mode;
pub(crate) mod classify_item;
pub(crate) mod do_now_list_single_item;
pub(crate) mod parent_back_to_a_motivation;
pub(crate) mod pick_item_review_frequency;
pub(crate) mod pick_what_should_be_done_first;
pub(crate) mod review_item;
pub(crate) mod search;

use std::{fmt::Display, iter::once};

use crate::menu::inquire::default_select_page_size;
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
use review_item::present_review_item_menu;
use search::present_search_menu;
use surrealdb::RecordId;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::BaseData,
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
        event_node::EventNode,
        item_status::ItemStatus,
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
    DeclareEvent { waiting_on: Vec<&'e EventNode<'e>> },
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
                bullet_list_created.format("%I:%M%P")
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
        event_nodes: &'a HashMap<&'a RecordId, EventNode<'a>>,
        do_now_list_created: DateTime<Utc>,
        current_mode: &'a CurrentMode,
    ) -> Vec<InquireDoNowListItem<'a>> {
        let waiting_on = event_nodes
            .values()
            .filter(|event_node| event_node.is_active())
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
    let do_now_list = load_do_now_list_from_db(send_to_data_storage_layer).await;
    // If the user previously said they're currently working on an item, resume directly into
    // that single-item view instead of showing the main list.
    if let Some(working_on) = do_now_list.get_base_data().get_surreal_working_on() {
        let resumed = do_now_list
            .get_all_items_status()
            .iter()
            .find_map(|(id, status)| {
                if *id == &working_on.item {
                    Some(status)
                } else {
                    None
                }
            });

        if let Some(item_status) = resumed {
            if item_status.is_finished() {
                send_to_data_storage_layer
                    .send(DataLayerCommands::ClearWorkingOn)
                    .await
                    .unwrap();
            } else {
                let mut why_in_scope = HashSet::default();
                why_in_scope.insert(WhyInScope::MenuNavigation);
                return Box::pin(present_do_now_list_item_selected(
                    item_status,
                    &why_in_scope,
                    Utc::now(),
                    &do_now_list,
                    send_to_data_storage_layer,
                ))
                .await;
            }
        } else {
            // Item was not found in the do_now_list, clear stale working_on state
            send_to_data_storage_layer
                .send(DataLayerCommands::ClearWorkingOn)
                .await
                .unwrap();
        }
    }

    present_upcoming(&do_now_list);
    present_do_now_list_menu(do_now_list, send_to_data_storage_layer).await
}

async fn load_do_now_list_from_db(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> DoNowList {
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

    do_now_list
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

enum EventSelection {
    ReturnToDoNowList,
    Event { event_id: RecordId, summary: String },
}

impl Display for EventSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSelection::ReturnToDoNowList => write!(f, "🔙 Return to Do Now List"),
            EventSelection::Event {
                event_id: _,
                summary,
            } => {
                write!(f, "{}", summary)
            }
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
    mut do_now_list: DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let ordered_do_now_list = do_now_list.get_ordered_do_now_list();
    let event_nodes = do_now_list.get_event_nodes();

    let inquire_do_now_list = InquireDoNowListItem::create_list(
        ordered_do_now_list,
        event_nodes,
        *do_now_list.get_now(),
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
            present_search_menu(&do_now_list, send_to_data_storage_layer).await
        }
        Ok(InquireDoNowListItem::ChangeMode(current_mode)) => {
            present_change_mode_menu(current_mode, send_to_data_storage_layer).await
        }
        Ok(InquireDoNowListItem::DeclareEvent { mut waiting_on }) => {
            waiting_on.sort_by(|a, b| b.get_last_updated().cmp(a.get_last_updated()));
            let list = chain!(
                once(EventSelection::ReturnToDoNowList),
                waiting_on.into_iter().map(|a| EventSelection::Event {
                    event_id: a.get_surreal_record_id().clone(),
                    summary: a.get_summary().to_string()
                })
            )
            .collect::<Vec<_>>();
            let selected = Select::new("Select the event that just happened|", list)
                .with_page_size(default_select_page_size())
                .prompt();
            match selected {
                Ok(EventSelection::Event {
                    event_id,
                    summary: _,
                }) => {
                    // Keep the user in the event-dependent-items list after working on an item.
                    // Rebuild the list from the database each time so removed dependencies/items
                    // don't show up stale.
                    loop {
                        let event_node = do_now_list.get_event_nodes().get(&event_id);

                        let Some(event_node) = event_node else {
                            // Event no longer exists (or no longer loaded). Exit back to Do Now list.
                            break Ok(());
                        };

                        // If nothing is waiting on this event anymore, return to the Do Now list.
                        if !event_node.is_active() {
                            break Ok(());
                        }

                        let mut items_waiting_on_this_event: Vec<&ItemStatus<'_>> =
                            event_node.get_waiting_on_this().to_vec();
                        //Order the list so it is the same each time you look at it and put the most recently created items at the top of the list
                        items_waiting_on_this_event
                            .sort_by(|a, b| b.get_created().cmp(a.get_created()));
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
                                // Clear the items' event dependency before triggering the event.
                                // (Clear dependency first in case triggering is canceled part way through.)
                                for item_waiting_on_event in all_items_waiting_on_event {
                                    send_to_data_storage_layer
                                        .send(DataLayerCommands::RemoveItemDependency(
                                            item_waiting_on_event.get_surreal_record_id().clone(),
                                            SurrealDependency::AfterEvent(event_id.clone()),
                                        ))
                                        .await
                                        .unwrap();
                                }
                                send_to_data_storage_layer
                                    .send(DataLayerCommands::TriggerEvent {
                                        event: event_id.clone(),
                                        when: Utc::now().into(),
                                    })
                                    .await
                                    .unwrap();
                                break Ok(());
                            }
                            Ok(EventTrigger::ItemDependentOnThisEvent(item_status)) => {
                                let mut why_in_scope = HashSet::default();
                                why_in_scope.insert(WhyInScope::MenuNavigation);

                                // After returning, loop back to this event list (but refreshed).
                                Box::pin(present_do_now_list_item_selected(
                                    item_status,
                                    &why_in_scope,
                                    Utc::now(),
                                    &do_now_list,
                                    send_to_data_storage_layer,
                                ))
                                .await?;
                            }
                            Ok(EventTrigger::ReturnToDoNowList)
                            | Err(InquireError::OperationCanceled) => break Ok(()),
                            Err(InquireError::OperationInterrupted) => break Err(()),
                            Err(err) => {
                                panic!("Unexpected error, try restarting the terminal: {}", err)
                            }
                        }
                        //Refresh the data for the next time around the loop so changes made by the user are reflected in the list

                        do_now_list = load_do_now_list_from_db(send_to_data_storage_layer).await;
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
                Box::pin(
                    pick_what_should_be_done_first::priority_wizard::priority_wizard_loop(
                        choices,
                        &do_now_list,
                        send_to_data_storage_layer,
                    ),
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
                                Utc::now(),
                                &do_now_list,
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

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            surreal_event::SurrealEvent,
            surreal_item::{SurrealDependency, SurrealItemBuilder, SurrealItemType},
            surreal_tables::SurrealTablesBuilder,
        },
        menu::inquire::do_now_list_menu::InquireDoNowListItem,
        node::urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
        systems::do_now_list::current_mode::CurrentMode,
    };

    #[test]
    fn declare_event_not_shown_when_all_items_waiting_on_event_are_finished() {
        let now = Utc::now();
        let event_id: surrealdb::RecordId = ("events", "1").into();

        let surreal_event = SurrealEvent {
            id: Some(event_id.clone()),
            version: 0,
            last_updated: now.into(),
            triggered: false,
            summary: "Some event".to_string(),
        };

        let finished_item_waiting_on_event = SurrealItemBuilder::default()
            .id(Some(("item", "1").into()))
            .summary("Finished item waiting on event")
            .item_type(SurrealItemType::Action)
            .finished(Some(now.into()))
            .dependencies(vec![SurrealDependency::AfterEvent(event_id)])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![finished_item_waiting_on_event])
            .surreal_events(vec![surreal_event])
            .build()
            .expect("no required fields");

        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let event_nodes = calculated_data.get_event_nodes();
        let do_now_list_created = now;
        let current_mode = CurrentMode::default();

        let empty_actions: [UrgencyLevelItemWithItemStatus; 0] = [];
        let list = InquireDoNowListItem::create_list(
            &empty_actions,
            event_nodes,
            do_now_list_created,
            &current_mode,
        );

        assert!(
            !list
                .iter()
                .any(|x| matches!(x, InquireDoNowListItem::DeclareEvent { .. })),
            "DeclareEvent should not be shown when no active items wait on any event"
        );
    }
}
