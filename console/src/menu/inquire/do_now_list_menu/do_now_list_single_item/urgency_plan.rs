use ahash::HashSet;
use chrono::{DateTime, Utc};
use fundu::{CustomDurationParser, CustomTimeUnit, SaturatingInto, TimeUnit};
use lazy_static::lazy_static;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, event::Event},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        SurrealItemsInScope, SurrealTrigger,
        data_layer_commands::DataLayerCommands,
        surreal_item::{SurrealDependency, SurrealScheduled, SurrealUrgency, SurrealUrgencyPlan},
        surreal_tables::SurrealTables,
    },
    display::{
        display_dependencies_with_item_node::DisplayDependenciesWithItemNode,
        display_item_node::DisplayFormat,
    },
    menu::inquire::{
        do_now_list_menu::do_now_list_single_item::state_a_smaller_action::{
            SelectAnItemSortingOrder, ShowCreateNewItem, select_an_item,
        },
        parse_exact_or_relative_datetime, parse_exact_or_relative_datetime_help_string,
        prompt_for_mode_scope,
    },
    new_event::{NewEvent, NewEventBuilder},
    node::{
        Filter, Urgency,
        item_node::ItemNode,
        item_status::{DependencyWithItemNode, ItemStatus},
        mode_node::ModeNode,
        why_in_scope_and_action_with_item_status::WhyInScope,
    },
};
use inquire::{InquireError, Select, Text};
use itertools::chain;
use std::{
    fmt::{Display, Formatter},
    iter::once,
};

enum UrgencyPlanSelection {
    StaysTheSame,
    WillEscalate,
}

impl Display for UrgencyPlanSelection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UrgencyPlanSelection::StaysTheSame => write!(f, "Stays the same"),
            UrgencyPlanSelection::WillEscalate => write!(f, "Escalate at trigger"),
        }
    }
}

enum ReadySelection {
    Now,
    NothingElse,
    AfterDateTime,
    AfterItem,
    AfterEvent,
}

impl Display for ReadySelection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadySelection::Now => write!(f, "🗸 Ready, now"),
            ReadySelection::NothingElse => write!(f, "Nothing else"),
            ReadySelection::AfterDateTime => {
                write!(f, "✗ Wait until...an exact date/time")
            }
            ReadySelection::AfterItem => {
                write!(f, "✗ Wait until...another item finishes")
            }
            ReadySelection::AfterEvent => {
                write!(f, "✗ Wait until...an event happens")
            }
        }
    }
}

pub(crate) async fn prompt_for_dependencies_and_urgency_plan(
    currently_selected: Option<&ItemStatus<'_>>,
    calculated_data: &CalculatedData,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> (Vec<AddOrRemove>, SurrealUrgencyPlan) {
    let ready = prompt_for_dependencies(
        currently_selected,
        calculated_data.get_base_data(),
        send_to_data_storage_layer,
    )
    .await;
    let now = Utc::now();
    let blank = Vec::default();
    let head_parent_items = match currently_selected {
        Some(currently_selected) => currently_selected
            .get_head_parent_items(Filter::Active)
            .collect::<Vec<_>>(),
        None => blank,
    };
    let urgency_plan = prompt_for_urgency_plan(
        &now,
        calculated_data.get_mode_nodes(),
        head_parent_items.as_slice(),
        send_to_data_storage_layer,
    )
    .await;
    (ready.unwrap(), urgency_plan)
}

pub(crate) enum AddOrRemove {
    AddExisting(SurrealDependency),
    AddNewEvent(NewEvent),
    RemoveExisting(SurrealDependency),
}

enum RemoveOrKeep {
    Remove,
    Keep,
}

impl Display for RemoveOrKeep {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoveOrKeep::Remove => write!(f, "Remove"),
            RemoveOrKeep::Keep => write!(f, "Keep"),
        }
    }
}

enum EventSelection<'e> {
    NewEvent,
    ExistingEvent(&'e Event<'e>),
}

impl Display for EventSelection<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSelection::NewEvent => write!(f, "Create a new event"),
            EventSelection::ExistingEvent(event) => write!(f, "{}", event.get_summary()),
        }
    }
}

pub(crate) async fn prompt_for_dependencies(
    currently_selected: Option<&ItemStatus<'_>>,
    base_data: &BaseData,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<Vec<AddOrRemove>, ()> {
    let mut result = Vec::default();
    let mut user_choose_to_keep = false;
    if let Some(currently_selected) = currently_selected {
        let currently_waiting_on = currently_selected.get_dependencies(Filter::Active);
        for currently_waiting_on in currently_waiting_on {
            match currently_waiting_on {
                DependencyWithItemNode::AfterDateTime { .. }
                | DependencyWithItemNode::AfterItem(..)
                | DependencyWithItemNode::DuringItem(..)
                | DependencyWithItemNode::AfterEvent(..) => {
                    println!(
                        "{}",
                        DisplayDependenciesWithItemNode::new(
                            &vec![&currently_waiting_on],
                            Filter::Active,
                            DisplayFormat::SingleLine
                        )
                    );

                    let selection = Select::new(
                        "Do you want to keep or remove this dependency?",
                        vec![RemoveOrKeep::Keep, RemoveOrKeep::Remove],
                    )
                    .prompt()
                    .unwrap();
                    match selection {
                        RemoveOrKeep::Keep => {
                            //keep is default so do nothing
                            user_choose_to_keep = true;
                        }
                        RemoveOrKeep::Remove => {
                            result.push(AddOrRemove::RemoveExisting(
                                currently_waiting_on.clone().into(),
                            ));
                        }
                    }
                }
                DependencyWithItemNode::UntilScheduled { .. }
                | DependencyWithItemNode::AfterChildItem(..)
                | DependencyWithItemNode::WaitingToBeInterrupted => {
                    //Not stored in SurrealDependencies so just skip over
                }
            }
        }
    }
    let mut list = Vec::default();
    if user_choose_to_keep {
        list.push(ReadySelection::NothingElse);
    } else {
        list.push(ReadySelection::Now);
    }

    list.push(ReadySelection::AfterDateTime);
    list.push(ReadySelection::AfterItem);
    list.push(ReadySelection::AfterEvent);

    println!();
    let ready = Select::new("When will this item be ready to work on?", list).prompt();
    match ready {
        Ok(ReadySelection::Now | ReadySelection::NothingElse) => {
            //do nothing
        }
        Ok(ReadySelection::AfterDateTime) => {
            let exact_start: DateTime<Utc> = loop {
                println!();
                let exact_start = match Text::new(
                    "Enter a date or an amount of time to wait (\"?\" for help)\n|",
                )
                .prompt()
                {
                    Ok(exact_start) => exact_start,
                    Err(InquireError::OperationCanceled) => {
                        todo!("Go back to the previous menu");
                    }
                    Err(InquireError::OperationInterrupted) => {
                        return Err(());
                    }
                    Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
                };
                let exact_start = parse_exact_or_relative_datetime(&exact_start);
                match exact_start {
                    Some(exact_start) => break exact_start.into(),
                    None => {
                        println!("Invalid date or duration, please try again");
                        println!();
                        println!("{}", parse_exact_or_relative_datetime_help_string());
                        continue;
                    }
                }
            };
            result.push(AddOrRemove::AddExisting(SurrealDependency::AfterDateTime(
                exact_start.into(),
            )));
        }
        Ok(ReadySelection::AfterItem) => {
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            let now = Utc::now();
            let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
            let calculated_data = CalculatedData::new_from_base_data(base_data);
            let excluded = if let Some(currently_selected) = currently_selected {
                vec![currently_selected.get_item()]
            } else {
                vec![]
            };
            let selected = select_an_item(
                excluded,
                SelectAnItemSortingOrder::NewestFirst,
                &calculated_data,
                ShowCreateNewItem::Yes,
            )
            .await;
            match selected {
                Ok(Some(after_item)) => result.push(AddOrRemove::AddExisting(
                    SurrealDependency::AfterItem(after_item.get_surreal_record_id().clone()),
                )),
                Ok(None) => {
                    println!("Canceled");
                    todo!()
                }
                Err(()) => {
                    return Err(());
                }
            }
        }
        Ok(ReadySelection::AfterEvent) => {
            let events = base_data.get_events();
            let mut events = events.values().collect::<Vec<_>>();
            events.sort_by(|a, b| b.get_last_updated().cmp(a.get_last_updated()));
            let list = chain!(
                once(EventSelection::NewEvent),
                events.iter().map(|x| EventSelection::ExistingEvent(x))
            )
            .collect::<Vec<_>>();
            let selected = Select::new(
                "Select an event that must happen first or create a new event",
                list,
            )
            .prompt();
            match selected {
                Ok(EventSelection::NewEvent) => {
                    let new_event = Text::new("Enter the name of the new event")
                        .prompt()
                        .unwrap();
                    let new_event = NewEventBuilder::default()
                        .summary(new_event)
                        .build()
                        .unwrap();
                    result.push(AddOrRemove::AddNewEvent(new_event));
                }
                Ok(EventSelection::ExistingEvent(event)) => {
                    //If the event is triggered so we need to untrigger the event so it can be triggered again
                    //But we also do this even if the event is not triggered because it also updates the last updated time
                    send_to_data_storage_layer
                        .send(DataLayerCommands::UntriggerEvent {
                            event: event.get_surreal_record_id().clone(),
                            when: Utc::now().into(),
                        })
                        .await
                        .unwrap();
                    let event =
                        SurrealDependency::AfterEvent(event.get_surreal_record_id().clone());
                    result.push(AddOrRemove::AddExisting(event));
                }
                Err(InquireError::OperationCanceled) => {
                    todo!()
                }
                Err(InquireError::OperationInterrupted) => {
                    return Err(());
                }
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }
        Err(InquireError::OperationCanceled) => todo!(),
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };
    Ok(result)
}

pub(crate) async fn prompt_for_urgency_plan(
    now: &DateTime<Utc>,
    all_modes: &[ModeNode<'_>],
    head_parent_items: &[&ItemNode<'_>],
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> SurrealUrgencyPlan {
    println!("Initial Urgency");
    let initial_urgency = prompt_for_urgency(all_modes, head_parent_items);

    let urgency_plan = Select::new(
        "Does the urgency escalate?|",
        vec![
            UrgencyPlanSelection::StaysTheSame,
            UrgencyPlanSelection::WillEscalate,
        ],
    )
    .prompt()
    .unwrap();

    match urgency_plan {
        UrgencyPlanSelection::StaysTheSame => SurrealUrgencyPlan::StaysTheSame(initial_urgency),
        UrgencyPlanSelection::WillEscalate => {
            let triggers = prompt_for_triggers(now, send_to_data_storage_layer).await;

            println!("Later Urgency");
            let later_urgency = prompt_for_urgency(all_modes, head_parent_items);

            SurrealUrgencyPlan::WillEscalate {
                initial: initial_urgency,
                triggers,
                later: later_urgency,
            }
        }
    }
}

enum TriggerType {
    WallClockDateTime,
    LoggedInvocationCount,
    LoggedAmountOfTimeSpent,
}

impl Display for TriggerType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerType::WallClockDateTime => write!(f, "After a wall clock date time"),
            TriggerType::LoggedInvocationCount => write!(f, "After a logged invocation count"),
            TriggerType::LoggedAmountOfTimeSpent => {
                write!(f, "After a logged amount of time spent")
            }
        }
    }
}

enum AddAnotherTrigger {
    AllDone,
    AddAnother,
}

impl Display for AddAnotherTrigger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AddAnotherTrigger::AllDone => write!(f, "Done adding triggers (recommended)"),
            AddAnotherTrigger::AddAnother => {
                write!(f, "Add another trigger, (only one trigger needs to happen)")
            }
        }
    }
}

pub(crate) async fn prompt_for_triggers(
    now: &DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Vec<SurrealTrigger> {
    let mut result = Vec::default();
    loop {
        let trigger = prompt_for_trigger(now, send_to_data_storage_layer).await;
        result.push(trigger);
        let more = Select::new(
            "Is there anything else that should also trigger?",
            vec![AddAnotherTrigger::AllDone, AddAnotherTrigger::AddAnother],
        )
        .prompt()
        .unwrap();
        match more {
            AddAnotherTrigger::AllDone => break,
            AddAnotherTrigger::AddAnother => continue,
        }
    }

    result
}

async fn prompt_for_trigger(
    now: &DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> SurrealTrigger {
    let trigger_type = Select::new(
        "What type of trigger?",
        vec![
            TriggerType::WallClockDateTime,
            TriggerType::LoggedInvocationCount,
            TriggerType::LoggedAmountOfTimeSpent,
        ],
    )
    .prompt()
    .unwrap();

    match trigger_type {
        TriggerType::WallClockDateTime => loop {
            let exact_start =
                match Text::new("Enter when you want to trigger (\"?\" for help)\n|").prompt() {
                    Ok(exact_start) => exact_start,
                    Err(InquireError::OperationCanceled) => {
                        todo!("Go back to the previous menu");
                    }
                    Err(InquireError::OperationInterrupted) => {
                        todo!("Change return type of this function so this can be returned")
                    }
                    Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
                };
            let exact_start: DateTime<Utc> = match parse_exact_or_relative_datetime(&exact_start) {
                Some(exact_start) => exact_start.into(),
                None => {
                    println!("Invalid date or duration, please try again");
                    println!();
                    println!("{}", parse_exact_or_relative_datetime_help_string());
                    continue;
                }
            };
            break SurrealTrigger::WallClockDateTime(exact_start.into());
        },
        TriggerType::LoggedInvocationCount => {
            let count_needed = Text::new("Enter the count needed").prompt().unwrap();
            let count_needed = match count_needed.parse::<u32>() {
                Ok(count_needed) => count_needed,
                Err(e) => {
                    todo!("Error: {:?}", e)
                }
            };
            let items_in_scope = prompt_for_items_in_scope(send_to_data_storage_layer).await;

            SurrealTrigger::LoggedInvocationCount {
                starting: (*now).into(),
                count: count_needed,
                items_in_scope,
            }
        }
        TriggerType::LoggedAmountOfTimeSpent => {
            lazy_static! {
                static ref relative_parser: CustomDurationParser<'static> =
                    CustomDurationParser::builder()
                        .allow_time_unit_delimiter()
                        .number_is_optional()
                        .time_units(&[
                            CustomTimeUnit::with_default(
                                TimeUnit::Second,
                                &["s", "sec", "secs", "second", "seconds"]
                            ),
                            CustomTimeUnit::with_default(
                                TimeUnit::Minute,
                                &["m", "min", "mins", "minute", "minutes"]
                            ),
                            CustomTimeUnit::with_default(TimeUnit::Hour, &["h", "hour", "hours"]),
                        ])
                        .build();
            }

            let amount_of_time = loop {
                let amount_of_time = Text::new("Enter the amount of time (Examples:\"30sec\", \"30s\", \"30min\", \"30m\", \"2hours\", \"2h\")\n|").prompt().unwrap();
                match relative_parser.parse(&amount_of_time) {
                    Ok(amount_of_time) => break amount_of_time.saturating_into(),
                    Err(_) => {
                        println!("Invalid date or duration, please try again");
                        println!();
                        continue;
                    }
                }
            };
            let items_in_scope = prompt_for_items_in_scope(send_to_data_storage_layer).await;

            SurrealTrigger::LoggedAmountOfTime {
                starting: (*now).into(),
                duration: amount_of_time.into(),
                items_in_scope,
            }
        }
    }
}

enum ItemInScopeSelection {
    All,
    Include,
    Exclude,
}

impl Display for ItemInScopeSelection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemInScopeSelection::All => write!(f, "Any/all items"),
            ItemInScopeSelection::Include => write!(f, "Declare items to include"),
            ItemInScopeSelection::Exclude => write!(f, "Declare items to exclude"),
        }
    }
}

async fn prompt_for_items_in_scope(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> SurrealItemsInScope {
    let selection = Select::new(
        "What items are in scope?",
        vec![
            ItemInScopeSelection::All,
            ItemInScopeSelection::Include,
            ItemInScopeSelection::Exclude,
        ],
    )
    .prompt()
    .unwrap();

    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let calculated_data = CalculatedData::new_from_base_data(base_data);

    match selection {
        ItemInScopeSelection::All => SurrealItemsInScope::All,
        ItemInScopeSelection::Include => {
            let selected_items = prompt_for_items_to_select(&calculated_data).await;

            SurrealItemsInScope::Include(
                selected_items
                    .into_iter()
                    .map(|x| x.get_surreal_record_id().clone())
                    .collect(),
            )
        }
        ItemInScopeSelection::Exclude => {
            let selected_items = prompt_for_items_to_select(&calculated_data).await;

            SurrealItemsInScope::Exclude(
                selected_items
                    .into_iter()
                    .map(|x| x.get_surreal_record_id().clone())
                    .collect(),
            )
        }
    }
}

enum SelectAnother {
    SelectAnother,
    Done,
}

impl Display for SelectAnother {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectAnother::SelectAnother => write!(f, "Select another"),
            SelectAnother::Done => write!(f, "Done"),
        }
    }
}

async fn prompt_for_items_to_select(calculated_data: &CalculatedData) -> Vec<&ItemStatus<'_>> {
    let mut result: Vec<&ItemStatus> = Vec::default();

    loop {
        let dont_show_these_items = result.iter().map(|x| x.get_item()).collect();
        let selected = select_an_item(
            dont_show_these_items,
            SelectAnItemSortingOrder::MotivationsFirst,
            calculated_data,
            ShowCreateNewItem::Yes,
        )
        .await
        .unwrap()
        .unwrap();

        result.push(selected);

        let select_another = Select::new(
            "Do you want to select another item?",
            vec![SelectAnother::SelectAnother, SelectAnother::Done],
        )
        .prompt()
        .unwrap();
        match select_another {
            SelectAnother::SelectAnother => {
                //do nothing, continue
            }
            SelectAnother::Done => {
                break;
            }
        }
    }
    result
}

impl Display for Urgency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Urgency::Crises => {
                write!(f, "🔥 Crises urgency")
            }
            Urgency::Scheduled => write!(f, "🗓️ Scheduled"),
            Urgency::DefinitelyUrgent => {
                write!(f, "🔴 Definitely urgent")
            }
            Urgency::MaybeUrgent => write!(f, "🟡 Maybe urgent"),
            Urgency::NotUrgent => write!(f, "🟢 Not urgent"),
        }
    }
}

fn prompt_for_urgency(
    all_modes: &[ModeNode<'_>],
    head_parent_items: &[&ItemNode<'_>],
) -> Option<SurrealUrgency> {
    let urgency = Select::new(
        "Select immediate urgency|",
        vec![
            Urgency::Crises,
            Urgency::Scheduled,
            Urgency::DefinitelyUrgent,
            Urgency::MaybeUrgent,
            Urgency::NotUrgent,
        ],
    )
    .with_starting_cursor(4)
    .prompt()
    .unwrap();
    let mode = prompt_for_mode_scope(all_modes, head_parent_items);
    match urgency {
        Urgency::Crises => Some(SurrealUrgency::CrisesUrgent(mode)),
        Urgency::Scheduled => Some(SurrealUrgency::Scheduled(
            mode,
            prompt_to_schedule().unwrap().unwrap(),
        )),
        Urgency::DefinitelyUrgent => Some(SurrealUrgency::DefinitelyUrgent(mode)),
        Urgency::MaybeUrgent => Some(SurrealUrgency::MaybeUrgent(mode)),
        Urgency::NotUrgent => None,
    }
}

enum StartWhenOption {
    ExactTime,
    TimeRange,
}

impl Display for StartWhenOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StartWhenOption::ExactTime => write!(f, "Exact Time"),
            StartWhenOption::TimeRange => write!(f, "Time Range"),
        }
    }
}

pub(crate) enum StartWhen {
    ExactTime(DateTime<Utc>),
    TimeRange(DateTime<Utc>, DateTime<Utc>),
}

fn prompt_to_schedule() -> Result<Option<SurrealScheduled>, ()> {
    let start_when = vec![StartWhenOption::ExactTime, StartWhenOption::TimeRange];
    let start_when = Select::new("When do you want to start this item?", start_when).prompt();
    let start_when = match start_when {
        Ok(StartWhenOption::ExactTime) => loop {
            let exact_start =
                Text::new("Enter the exact time you want to start this item (\"?\" for help)\n|")
                    .prompt()
                    .unwrap();

            let exact_start = match parse_exact_or_relative_datetime(&exact_start) {
                Some(exact_start) => exact_start,
                None => {
                    println!("Invalid date or duration, please try again");
                    println!();
                    println!("{}", parse_exact_or_relative_datetime_help_string());
                    continue;
                }
            };
            break StartWhen::ExactTime(exact_start.into());
        },
        Ok(StartWhenOption::TimeRange) => {
            let range_start = loop {
                let range_start =
                    match Text::new("Enter the start of the range (\"?\" for help)\n|").prompt() {
                        Ok(range_start) => match parse_exact_or_relative_datetime(&range_start) {
                            Some(range_start) => range_start,
                            None => {
                                println!("Invalid date or duration, please try again");
                                println!();
                                println!("{}", parse_exact_or_relative_datetime_help_string());
                                continue;
                            }
                        },
                        Err(InquireError::OperationCanceled) => {
                            todo!();
                        }
                        Err(InquireError::OperationInterrupted) => return Err(()),
                        Err(err) => {
                            panic!("Unexpected error, try restarting the terminal: {}", err)
                        }
                    };
                break range_start.into();
            };
            let range_end = loop {
                let range_end = match Text::new("Enter the end of the range (\"?\" for help)\n|")
                    .prompt()
                {
                    Ok(range_end) => match parse_exact_or_relative_datetime(&range_end) {
                        Some(range_end) => range_end,
                        None => {
                            println!("Invalid date or duration, please try again");
                            println!();
                            println!("{}", parse_exact_or_relative_datetime_help_string());
                            continue;
                        }
                    },
                    Err(InquireError::OperationCanceled) => {
                        todo!();
                    }
                    Err(InquireError::OperationInterrupted) => return Err(()),
                    Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
                };
                break range_end.into();
            };
            StartWhen::TimeRange(range_start, range_end)
        }
        Err(InquireError::OperationCanceled) => {
            println!("Operation canceled");
            return Ok(None);
        }
        Err(InquireError::OperationInterrupted) => return Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    };

    lazy_static! {
        static ref relative_parser: CustomDurationParser<'static> = CustomDurationParser::builder()
            .allow_time_unit_delimiter()
            .number_is_optional()
            .time_units(&[
                CustomTimeUnit::with_default(
                    TimeUnit::Second,
                    &["s", "sec", "secs", "second", "seconds"]
                ),
                CustomTimeUnit::with_default(
                    TimeUnit::Minute,
                    &["m", "min", "mins", "minute", "minutes"]
                ),
                CustomTimeUnit::with_default(TimeUnit::Hour, &["h", "hour", "hours"]),
            ])
            .build();
    }

    let time_boxed = loop {
        let time_boxed = Text::new("Time box how much time for this item (Examples: \"30sec\", \"30s\", \"30min\", \"30m\", \"2hours\", \"2h\")")
        .prompt()
        .unwrap();
        match relative_parser.parse(&time_boxed) {
            Ok(time_boxed) => break time_boxed.saturating_into(),
            Err(_) => {
                println!("Invalid date or duration, please try again");
                println!();
                continue;
            }
        };
    };

    let surreal_scheduled = match start_when {
        StartWhen::ExactTime(exact_start) => SurrealScheduled::Exact {
            start: exact_start.into(),
            duration: time_boxed.into(),
        },
        StartWhen::TimeRange(range_start, range_end) => SurrealScheduled::Range {
            start_range: (range_start.into(), range_end.into()),
            duration: time_boxed.into(),
        },
    };

    Ok(Some(surreal_scheduled))
}

pub(crate) async fn present_set_ready_and_urgency_plan_menu(
    selected: &ItemStatus<'_>,
    why_in_scope: &HashSet<WhyInScope>,
    current_urgency: Option<SurrealUrgency>,
    calculated_data: &CalculatedData,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let (dependencies, urgency_plan) = prompt_for_dependencies_and_urgency_plan(
        Some(selected),
        calculated_data,
        send_to_data_storage_layer,
    )
    .await;

    for command in dependencies.into_iter() {
        match command {
            AddOrRemove::AddExisting(dependency) => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::AddItemDependency(
                        selected.get_surreal_record_id().clone(),
                        dependency,
                    ))
                    .await
                    .unwrap();
            }
            AddOrRemove::RemoveExisting(dependency) => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::RemoveItemDependency(
                        selected.get_surreal_record_id().clone(),
                        dependency,
                    ))
                    .await
                    .unwrap();
            }
            AddOrRemove::AddNewEvent(new_event) => {
                send_to_data_storage_layer
                    .send(DataLayerCommands::AddItemDependencyNewEvent(
                        selected.get_surreal_record_id().clone(),
                        new_event,
                    ))
                    .await
                    .unwrap();
            }
        }
    }

    send_to_data_storage_layer
        .send(DataLayerCommands::UpdateUrgencyPlan(
            selected.get_surreal_record_id().clone(),
            Some(urgency_plan),
        ))
        .await
        .unwrap();

    Ok(())
}
