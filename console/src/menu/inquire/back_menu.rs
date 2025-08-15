pub(crate) mod configure_modes;
pub(crate) mod configure_settings;

use std::{cmp::Ordering, fmt::Display, vec};

use ahash::HashMap;
use chrono::{DateTime, Local, Utc};
use configure_settings::configure_settings;
use inquire::{InquireError, Select, Text};
use surrealdb::opt::RecordId;
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item, time_spent::TimeSpent},
    calculated_data::{CalculatedData, parent_lookup::ParentLookup},
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::{
        display_duration::DisplayDuration,
        display_item::DisplayItem,
        display_item_node::{DisplayFormat, DisplayItemNode},
        display_item_status::DisplayItemStatus,
    },
    menu::inquire::{
        back_menu::configure_modes::configure_modes, parse_exact_or_relative_datetime,
        parse_exact_or_relative_datetime_help_string,
    },
    new_item::NewItem,
    node::{
        Filter,
        item_node::{ItemNode, ShrinkingItemNode},
        item_status::ItemStatus,
    },
};

use super::{
    do_now_list_menu::present_normal_do_now_list_menu, update_item_summary::update_item_summary,
};

enum TopMenuSelection {
    Reflection,
    ViewDoNowList,
    ViewImportancePriorities,
    ClearInTheMomentPriorities,
    ConfigureModes,
    ConfigureSettings,
    DebugViewAllItems,
}

impl Display for TopMenuSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopMenuSelection::Reflection => write!(f, "🤔  Reflection, what I did"),
            TopMenuSelection::ViewDoNowList => {
                write!(f, "🔙  Return to Do Now List")
            }
            TopMenuSelection::ViewImportancePriorities => {
                write!(f, "⚖️  View Importance Priorities")
            }
            TopMenuSelection::DebugViewAllItems => {
                write!(f, "🔍  Debug View All Items")
            }
            TopMenuSelection::ConfigureSettings => write!(f, "⚙️  Configure Settings"),
            TopMenuSelection::ConfigureModes => write!(f, "😊  Configure Modes"),
            TopMenuSelection::ClearInTheMomentPriorities => {
                write!(f, "🗑️  Clear In The Moment Priorities")
            }
        }
    }
}

impl TopMenuSelection {
    fn make_list() -> Vec<TopMenuSelection> {
        vec![
            Self::ViewImportancePriorities,
            Self::ClearInTheMomentPriorities,
            Self::Reflection,
            Self::ConfigureModes,
            Self::ConfigureSettings,
            Self::ViewDoNowList,
            Self::DebugViewAllItems,
        ]
    }
}

pub(crate) async fn present_back_menu(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let back_menu = TopMenuSelection::make_list();

    println!();
    let selection = Select::new("Select from the below list|", back_menu).prompt();
    match selection {
        Ok(TopMenuSelection::Reflection) => present_reflection(send_to_data_storage_layer).await,
        Ok(TopMenuSelection::ViewDoNowList) => {
            present_normal_do_now_list_menu(send_to_data_storage_layer).await
        }
        Ok(TopMenuSelection::ViewImportancePriorities) => {
            view_priorities(send_to_data_storage_layer).await
        }
        Ok(TopMenuSelection::ClearInTheMomentPriorities) => {
            clear_in_the_moment_priorities(send_to_data_storage_layer).await
        }
        Ok(TopMenuSelection::ConfigureSettings) => configure_settings().await,
        Ok(TopMenuSelection::ConfigureModes) => configure_modes(send_to_data_storage_layer).await,
        Ok(TopMenuSelection::DebugViewAllItems) => {
            debug_view_all_items(send_to_data_storage_layer).await
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) async fn capture(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let new_item_summary = Text::new("Enter New Item ⍠").prompt();

    match new_item_summary {
        Ok(new_item_summary) => {
            let new_item = NewItem::new(new_item_summary, Utc::now());
            send_to_data_storage_layer
                .send(DataLayerCommands::NewItem(new_item))
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

enum ClearInTheMomentPrioritiesChoice {
    ClearAll,
    Back,
}

impl Display for ClearInTheMomentPrioritiesChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClearInTheMomentPrioritiesChoice::ClearAll => write!(f, "Clear All"),
            ClearInTheMomentPrioritiesChoice::Back => write!(f, "Back"),
        }
    }
}

async fn clear_in_the_moment_priorities(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let menu_choices = vec![
        ClearInTheMomentPrioritiesChoice::ClearAll,
        ClearInTheMomentPrioritiesChoice::Back,
    ];
    let selection = Select::new("Select an action...", menu_choices).prompt();

    match selection {
        Ok(ClearInTheMomentPrioritiesChoice::ClearAll) => {
            print!("Clearing all in the moment priorities");
            print!("...");
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            for in_the_moment_priority in surreal_tables.get_surreal_in_the_moment_priorities() {
                print!(".");
                let in_the_moment_priority =
                    in_the_moment_priority.id.as_ref().expect("In DB").clone();
                send_to_data_storage_layer
                    .send(DataLayerCommands::ClearInTheMomentPriority(
                        in_the_moment_priority,
                    ))
                    .await
                    .unwrap();
            }
            println!("Done");
            Ok(())
        }
        Ok(ClearInTheMomentPrioritiesChoice::Back) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationCanceled) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

async fn view_priorities(send_to_data_storage_layer: &Sender<DataLayerCommands>) -> Result<(), ()> {
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
    let calculated_data = CalculatedData::new_from_base_data(base_data);

    let mut all_top_nodes = calculated_data
        .get_items_status()
        .values()
        .filter(|x| !x.is_finished())
        //Person or group items without a parent, meaning a reason for being on the list,
        // should be filtered out.
        .filter(|x| x.has_children(Filter::Active) && !x.has_parents(Filter::Active))
        .cloned()
        .collect::<Vec<_>>();

    all_top_nodes.sort_by(|a, b| {
        (if a.is_type_motivation() {
            if b.is_type_motivation() {
                Ordering::Equal
            } else {
                Ordering::Less
            }
        } else if b.is_type_motivation() {
            Ordering::Greater
        } else {
            Ordering::Equal
        })
        .then_with(|| a.get_summary().cmp(b.get_summary()))
    });

    let list = all_top_nodes
        .iter()
        .map(|x| DisplayItemStatus::new(x, Filter::Active, DisplayFormat::SingleLine))
        .collect::<Vec<_>>();

    println!();
    let selection = Select::new("Select a priority to view...", list).prompt();
    match selection {
        Ok(display_item_status) => {
            view_priorities_of_item_status(
                display_item_status,
                Vec::new(),
                &calculated_data,
                &now,
                send_to_data_storage_layer,
            )
            .await
        }
        Err(InquireError::OperationCanceled) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

async fn view_priorities_of_item_status(
    display_item_status: DisplayItemStatus<'_>,
    mut parent: Vec<DisplayItemStatus<'_>>,
    calculated_data: &CalculatedData,
    now: &DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    println!("{}", display_item_status);
    let item_status = display_item_status.get_item_status();
    println!("Active children (Is this in priority order?):");
    let list = item_status
        .get_children(Filter::Active)
        .map(|x| {
            let item_status = calculated_data
                .get_items_status()
                .get(x.get_surreal_record_id())
                .expect("Comes from this list so will be found");
            DisplayItemStatus::new(item_status, Filter::Active, DisplayFormat::SingleLine)
        })
        .collect();
    let selection = Select::new("Select a child to view...", list)
        .with_page_size(8)
        .prompt();
    match selection {
        Ok(display_priority) => {
            println!("{}", display_priority);
            parent.push(display_item_status);
            if display_priority.has_children(Filter::Active) {
                let display_item_status = DisplayItemStatus::new(
                    display_priority.get_item_status(),
                    Filter::Active,
                    DisplayFormat::SingleLine,
                );
                Box::pin(view_priorities_of_item_status(
                    display_item_status,
                    parent,
                    calculated_data,
                    now,
                    send_to_data_storage_layer,
                ))
                .await
            } else {
                view_priorities_single_item_no_children(
                    display_priority.get_item_status(),
                    parent,
                    calculated_data,
                    now,
                    send_to_data_storage_layer,
                )
                .await
            }
        }
        Err(InquireError::OperationCanceled) => {
            if parent.is_empty() {
                Box::pin(view_priorities(send_to_data_storage_layer)).await
            } else {
                let top_item = parent.pop().expect("is not empty so will always succeed");
                Box::pin(view_priorities_of_item_status(
                    top_item,
                    parent,
                    calculated_data,
                    now,
                    send_to_data_storage_layer,
                ))
                .await
            }
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

enum ViewPrioritiesSingleItemNoChildrenChoice {
    Back,
    Finish,
    EditSummary,
}

impl Display for ViewPrioritiesSingleItemNoChildrenChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewPrioritiesSingleItemNoChildrenChoice::Finish => write!(f, "Finish"),
            ViewPrioritiesSingleItemNoChildrenChoice::EditSummary => write!(f, "Edit Summary"),
            ViewPrioritiesSingleItemNoChildrenChoice::Back => write!(f, "Back"),
        }
    }
}

async fn view_priorities_single_item_no_children(
    item_status: &ItemStatus<'_>,
    mut parent: Vec<DisplayItemStatus<'_>>,
    calculated_data: &CalculatedData,
    now: &DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let choices = vec![
        ViewPrioritiesSingleItemNoChildrenChoice::Back,
        ViewPrioritiesSingleItemNoChildrenChoice::EditSummary,
        ViewPrioritiesSingleItemNoChildrenChoice::Finish,
    ];
    let selection = Select::new("Select an action...", choices).prompt();
    match selection {
        Ok(ViewPrioritiesSingleItemNoChildrenChoice::Finish) => {
            let now = Utc::now();
            send_to_data_storage_layer
                .send(DataLayerCommands::FinishItem {
                    item: item_status.get_surreal_record_id().clone(),
                    when_finished: now.into(),
                })
                .await
                .unwrap();
            Ok(())
        }
        Ok(ViewPrioritiesSingleItemNoChildrenChoice::EditSummary) => {
            update_item_summary(item_status.get_item(), send_to_data_storage_layer).await
        }
        Err(InquireError::OperationCanceled)
        | Ok(ViewPrioritiesSingleItemNoChildrenChoice::Back) => {
            let top_item = parent.pop().expect("is not empty so will always succeed");
            Box::pin(view_priorities_of_item_status(
                top_item,
                parent,
                calculated_data,
                now,
                send_to_data_storage_layer,
            ))
            .await
        }
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

#[allow(clippy::mutable_key_type)]
async fn present_reflection(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let start = loop {
        match Text::new("Enter Staring Time (\"?\" for help)").prompt() {
            Ok(when_started) => match parse_exact_or_relative_datetime(&when_started) {
                Some(start) => break start,
                None => {
                    println!("Invalid input. Please try again.");
                    println!();
                    println!("{}", parse_exact_or_relative_datetime_help_string());
                    continue;
                }
            },
            Err(InquireError::OperationCanceled) => return Ok(()),
            Err(InquireError::OperationInterrupted) => return Err(()),
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        };
    };

    let end = loop {
        match Text::new("Enter Ending Time (\"?\" for help \"0m\" for now)").prompt() {
            Ok(when_finished) => match parse_exact_or_relative_datetime(&when_finished) {
                Some(end) => break end,
                None => {
                    println!("Invalid input. Please try again.");
                    println!();
                    println!("{}", parse_exact_or_relative_datetime_help_string());
                    continue;
                }
            },
            Err(InquireError::OperationCanceled) => {
                return Box::pin(present_reflection(send_to_data_storage_layer)).await;
            }
            Err(InquireError::OperationInterrupted) => return Err(()),
            Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
        };
    };

    println!("Time spent between {} and {}", start, end);

    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();

    let start_utc = start.with_timezone(&Utc);
    let end_utc = end.with_timezone(&Utc);

    let logs_in_range: Vec<_> = surreal_tables
        .make_time_spent_log()
        .filter(|x| x.is_within(&start_utc, &end_utc))
        .collect();

    let mut things_done: HashMap<RecordId, Vec<&TimeSpent>> = HashMap::default();
    for log in logs_in_range.iter() {
        for worked_towards in log.worked_towards().iter() {
            let h = things_done.entry(worked_towards.clone()).or_default();
            if !h.contains(&log) {
                h.push(log);
            }
        }
    }

    let base_data = BaseData::new_from_surreal_tables(surreal_tables.clone(), Utc::now());
    let calculated_data = CalculatedData::new_from_base_data(base_data);
    let items_status = calculated_data.get_items_status();

    let mut items_in_range: HashMap<&RecordId, ItemTimeSpent<'_>> = things_done
        .into_iter()
        .map(|(k, v)| {
            let item_status = items_status
                .get(&k)
                .expect("All items in the log should be in the item status");
            (
                item_status.get_surreal_record_id(),
                ItemTimeSpent {
                    item_status,
                    time_spent: v,
                    visited: false,
                },
            )
        })
        .collect();

    let neither = items_in_range
        .iter()
        .filter_map(|(_, v)| {
            if v.item_status.has_parents(Filter::All) {
                None
            } else if v.item_status.is_type_motivation_kind_neither() {
                Some(v.get_surreal_record_id().clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if !neither.is_empty() {
        println!("🚫 Neither Core nor Non-Core Work");
        println!();
        for item in neither.into_iter() {
            print_children_time_spent(item, &mut items_in_range);
        }
    }
    let no_parents_non_core: Vec<RecordId> = items_in_range
        .iter()
        .filter_map(|(_, v)| {
            if v.item_status.has_parents(Filter::All) {
                None
            } else if !v.is_type_motivation_kind_core() && !v.is_type_motivation_kind_neither() {
                Some(v.get_surreal_record_id().clone())
            } else {
                None
            }
        })
        .collect();

    println!("🧹 Non-Core Work");
    println!();
    for item in no_parents_non_core.into_iter() {
        print_children_time_spent(item, &mut items_in_range);
    }

    println!();
    println!("🏢 Core Work");
    println!();

    let no_parents_core: Vec<RecordId> = items_in_range
        .iter()
        .filter_map(|(_, v)| {
            if v.item_status.has_parents(Filter::All) {
                None
            } else if v.is_type_motivation_kind_core() {
                Some(v.get_surreal_record_id().clone())
            } else {
                None
            }
        })
        .collect();

    for item in no_parents_core.into_iter() {
        print_children_time_spent(item, &mut items_in_range);
    }

    println!();

    let not_visited = items_in_range
        .iter()
        .filter(|(_, x)| !x.visited)
        .map(|(_, x)| x.get_surreal_record_id().clone())
        .collect::<Vec<_>>();
    if !not_visited.is_empty() {
        println!("Not known if core or non-core because of shifting parent/child relationship");
        println!();
        for record_id in not_visited.into_iter() {
            print_children_time_spent(record_id, &mut items_in_range);
        }
        println!();
    }

    println!();

    let core_work = items_in_range
        .iter()
        .filter(|(_, x)| x.is_type_motivation_kind_core())
        .flat_map(|(_, x)| &x.time_spent)
        .fold(
            (chrono::Duration::default(), 0),
            |(sum_duration, count), time_spent| {
                (sum_duration + time_spent.get_time_delta(), count + 1)
            },
        );

    let non_core_work = items_in_range
        .iter()
        .filter(|(_, x)| x.is_type_motivation_kind_non_core())
        .flat_map(|(_, x)| &x.time_spent)
        .fold(
            (chrono::Duration::default(), 0),
            |(sum_duration, count), time_spent| {
                (sum_duration + time_spent.get_time_delta(), count + 1)
            },
        );

    let neither_work = items_in_range
        .iter()
        .filter(|(_, x)| x.is_type_motivation_kind_neither())
        .flat_map(|(_, x)| &x.time_spent)
        .fold(
            (chrono::Duration::default(), 0),
            |(sum_duration, count), time_spent| {
                (sum_duration + time_spent.get_time_delta(), count + 1)
            },
        );

    let total_time = logs_in_range
        .iter()
        .map(|x| x.get_time_delta())
        .sum::<chrono::Duration>();

    let total_time_num_seconds = total_time.num_seconds();
    let neither_work_num_seconds = neither_work.0.num_seconds();
    let core_and_non_core_num_seconds = total_time_num_seconds - neither_work_num_seconds;
    if core_and_non_core_num_seconds != 0 {
        println!("Core Work");
        println!(
            "\t{} times for {} ({}%)",
            core_work.1,
            DisplayDuration::new(&core_work.0.to_std().expect("valid")),
            core_work.0.num_seconds() * 100 / core_and_non_core_num_seconds
        );

        println!("Non-Core Work");
        println!(
            "\t{} times for {} ({}%)",
            non_core_work.1,
            DisplayDuration::new(&non_core_work.0.to_std().expect("valid")),
            non_core_work.0.num_seconds() * 100 / core_and_non_core_num_seconds
        );
    }

    if neither_work.0.num_seconds() != 0 {
        println!("Neither Core nor Non-Core Work");
        println!(
            "\t{} times for {}",
            neither_work.1,
            DisplayDuration::new(&neither_work.0.to_std().expect("valid"))
        );
    }

    let urgent_time = logs_in_range
        .iter()
        .filter(|x| x.is_urgent())
        .map(|x| x.get_time_delta())
        .sum::<chrono::Duration>();
    let most_important_time = logs_in_range
        .iter()
        .filter(|x| x.is_important())
        .map(|x| x.get_time_delta())
        .sum::<chrono::Duration>();
    let menu_selection_time = logs_in_range
        .iter()
        .filter(|x| x.is_menu_navigation())
        .map(|x| x.get_time_delta())
        .sum::<chrono::Duration>();

    if total_time_num_seconds != 0 {
        println!();
        if !urgent_time.is_zero() {
            println!(
                "Urgent time spent: {} ({}%)",
                DisplayDuration::new(&urgent_time.to_std().expect("valid")),
                urgent_time.num_seconds() * 100 / total_time_num_seconds
            );
        }
        if !most_important_time.is_zero() {
            println!(
                "Most important time spent: {} ({}%)",
                DisplayDuration::new(&most_important_time.to_std().expect("valid")),
                most_important_time.num_seconds() * 100 / total_time_num_seconds
            );
        }
        if !menu_selection_time.is_zero() {
            println!(
                "Menu selection time spent: {} ({}%)",
                DisplayDuration::new(&menu_selection_time.to_std().expect("valid")),
                menu_selection_time.num_seconds() * 100 / total_time_num_seconds
            );
        }
        println!(
            "Total time spent: {}",
            DisplayDuration::new(&total_time.to_std().expect("valid"))
        );
    }

    println!();
    match Text::new("Press Enter to continue...").prompt() {
        Ok(_) | Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

fn print_children_time_spent(
    record_id: RecordId,
    items_in_range: &mut HashMap<&RecordId, ItemTimeSpent>,
) {
    let mut visited = Vec::default();
    let item_time = items_in_range.get(&record_id).expect("Is there");
    let iteration_count = item_time.time_spent.len();
    let total_time: chrono::Duration = item_time
        .time_spent
        .iter()
        .map(|x| x.get_time_delta())
        .sum();
    let total_time: std::time::Duration = total_time.to_std().expect("valid");
    let display_duration = DisplayDuration::new(&total_time);
    println!(
        "{} 🕜🕜{} times for {}",
        DisplayItem::new(item_time.get_item()),
        iteration_count,
        display_duration
    );

    let children = item_time
        .get_item_node()
        .create_child_chain_filtered(Filter::All, items_in_range);
    for (j, (depth, item)) in children.iter().enumerate() {
        for k in 0..2 {
            //This for loop is so we can print out the tree structure with spaces between the lines so it is easier to read
            for i in 0..*depth {
                if i == *depth - 1 {
                    if k == 0 {
                        println!("  ┃");
                    } else {
                        let item_time = items_in_range
                            .get(item.get_surreal_record_id())
                            .expect("Is there");
                        visited.push(item_time.get_surreal_record_id().clone());
                        let iteration_count = item_time.time_spent.len();
                        let total_time: chrono::Duration = item_time
                            .time_spent
                            .iter()
                            .map(|x| x.get_time_delta())
                            .filter(|x| *x == x.abs())
                            .sum();
                        let total_time: std::time::Duration = total_time
                            .to_std()
                            .expect("We do filter to only positive values");
                        let display_duration = DisplayDuration::new(&total_time);

                        println!(
                            "  ┗{} 🕜🕜{} times for {}",
                            DisplayItem::new(item_time.get_item()),
                            iteration_count,
                            display_duration
                        );
                    }
                } else if children
                    .iter()
                    .skip(j + 1)
                    .take_while(|(d, _)| (*d - 1) >= i)
                    .any(|(d, _)| *d - 1 == i)
                {
                    print!("  ┃");
                } else {
                    print!("   ");
                }
            }
        }
    }
    println!();
    visited.push(record_id);
    for record_id in visited.into_iter() {
        items_in_range
            .get_mut(&record_id)
            .expect("Is there")
            .mark_as_visited();
    }
}

impl<'s> ItemNode<'s> {
    fn create_child_chain_filtered(
        &'s self,
        filter: Filter,
        filter_to: &HashMap<&RecordId, ItemTimeSpent>,
    ) -> Vec<(u32, &'s Item<'s>)> {
        let mut result = Vec::default();
        for i in self.get_children(filter) {
            if filter_to.contains_key(i.get_surreal_record_id()) {
                result.push((1, i.item));
                let children = i.create_shrinking_children_filtered(filter, 2, filter_to);
                result.extend(children.iter());
            }
        }
        result
    }
}

impl ShrinkingItemNode<'_> {
    fn create_shrinking_children_filtered(
        &self,
        filter: Filter,
        depth: u32,
        filter_to: &HashMap<&RecordId, ItemTimeSpent>,
    ) -> Vec<(u32, &Item<'_>)> {
        let mut result = Vec::default();
        for i in self.get_children(filter) {
            if filter_to.contains_key(i.get_surreal_record_id()) {
                result.push((depth, i.item));
                let children = i.create_shrinking_children_filtered(filter, depth + 1, filter_to);
                result.extend(children.iter());
            }
        }
        result
    }
}

struct ItemTimeSpent<'s> {
    item_status: &'s ItemStatus<'s>,
    time_spent: Vec<&'s TimeSpent<'s>>,
    visited: bool,
}

impl PartialEq for ItemTimeSpent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.item_status.get_item() == other.item_status.get_item()
    }
}

impl Eq for ItemTimeSpent<'_> {}

impl PartialOrd for ItemTimeSpent<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ItemTimeSpent<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_parent_count = self
            .item_status
            .get_self_and_parents_flattened(Filter::All)
            .len();
        let other_parent_count = other
            .item_status
            .get_self_and_parents_flattened(Filter::All)
            .len();
        if self_parent_count != other_parent_count {
            //Reverse order so that the item with the most parents is first
            other_parent_count.cmp(&self_parent_count)
        } else {
            self.item_status
                .get_summary()
                .cmp(other.item_status.get_summary())
        }
    }
}

impl ItemTimeSpent<'_> {
    fn is_type_motivation_kind_core(&self) -> bool {
        self.item_status.is_type_motivation_kind_core()
    }

    fn is_type_motivation_kind_non_core(&self) -> bool {
        self.item_status.is_type_motivation_kind_non_core()
    }

    fn is_type_motivation_kind_neither(&self) -> bool {
        self.item_status.is_type_motivation_kind_neither()
    }

    fn mark_as_visited(&mut self) {
        self.visited = true;
    }

    fn get_item_node(&self) -> &ItemNode<'_> {
        self.item_status.get_item_node()
    }

    fn get_item(&self) -> &Item<'_> {
        self.item_status.get_item()
    }

    fn get_surreal_record_id(&self) -> &RecordId {
        self.item_status.get_surreal_record_id()
    }
}

enum DebugViewItem<'e> {
    Item(DisplayItemNode<'e>),
}

impl Display for DebugViewItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebugViewItem::Item(item) => write!(f, "{}", item),
        }
    }
}

impl<'e> DebugViewItem<'e> {
    fn make_list(items: &'e [&'e ItemNode<'e>]) -> Vec<DebugViewItem<'e>> {
        items.iter().copied().map(DebugViewItem::new).collect()
    }

    fn new(item: &'e ItemNode<'e>) -> Self {
        Self::Item(DisplayItemNode::new(
            item,
            Filter::All,
            DisplayFormat::MultiLineTree,
        ))
    }
}

async fn debug_view_all_items(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();

    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let all_items = base_data.get_items();
    let all_parents = ParentLookup::new(all_items);
    let active_items = base_data.get_active_items();
    let all_events = base_data.get_events();
    let time_spent_log = base_data.get_time_spent_log();

    let item_nodes = active_items
        .iter()
        .map(|x| ItemNode::new(x, all_items, &all_parents, all_events, time_spent_log))
        .collect::<Vec<_>>();

    let item_nodes = item_nodes.iter().collect::<Vec<_>>();
    let list = DebugViewItem::make_list(&item_nodes);

    let selection = Select::new("Select an item to show the debug view of...", list)
        .with_page_size(8)
        .prompt();
    match selection {
        Ok(DebugViewItem::Item(item)) => {
            println!("{}", item);
            let item_node = item.get_item_node();
            println!("{:#?}", item_node);
            Ok(())
        }
        Err(InquireError::OperationCanceled) => {
            Box::pin(present_back_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}
