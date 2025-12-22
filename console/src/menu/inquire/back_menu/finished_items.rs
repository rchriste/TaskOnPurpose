use std::fmt::Display;

use chrono::Utc;
use inquire::{InquireError, Select};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::BaseData,
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands, surreal_tables::SurrealTables,
    },
    display::{
        display_duration::DisplayDuration,
        display_item::DisplayItem,
        display_item_node::{DisplayFormat, DisplayItemNode},
    },
    menu::inquire::default_select_page_size,
    node::{Filter, item_status::ItemStatus},
    systems::do_now_list::DoNowList,
};

use super::present_back_menu;

#[derive(Clone, Copy)]
struct FinishedItemListEntry<'s> {
    item_status: &'s ItemStatus<'s>,
}

impl Display for FinishedItemListEntry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let finished_at = self
            .item_status
            .get_finished_at()
            .as_ref()
            .map(|x| {
                let x_local = x.with_timezone(&chrono::Local);
                x_local.format("%a %d %b %Y %I:%M%p").to_string()
            })
            .unwrap_or_else(|| "(no finished date)".to_string());
        write!(
            f,
            "{}  {}",
            finished_at,
            DisplayItemNode::new(
                self.item_status.get_item_node(),
                Filter::All,
                DisplayFormat::SingleLine
            )
        )
    }
}

#[derive(Clone, Copy)]
enum FinishedItemDetailChoice {
    Reactivate,
    BackToFinishedList,
}

impl Display for FinishedItemDetailChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FinishedItemDetailChoice::Reactivate => write!(f, "Reactivate item"),
            FinishedItemDetailChoice::BackToFinishedList => write!(f, "Back to finished list"),
        }
    }
}

pub(crate) async fn present_finished_items_menu(
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let calculated_data = CalculatedData::new_from_base_data(base_data);
    let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

    let mut finished_items = do_now_list
        .get_all_items_status()
        .values()
        .filter(|x| x.is_finished())
        .collect::<Vec<_>>();

    finished_items.sort_by(|a, b| {
        b.get_item()
            .get_finished_at()
            .cmp(a.get_item().get_finished_at())
    });

    let list = finished_items
        .into_iter()
        .map(|item_status| FinishedItemListEntry { item_status })
        .collect::<Vec<_>>();

    println!();
    let selection = Select::new("Select a finished item...", list)
        .with_page_size(default_select_page_size())
        .prompt();

    match selection {
        Ok(selected) => {
            present_finished_item_detail(
                selected.item_status,
                &do_now_list,
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

async fn present_finished_item_detail(
    menu_for: &crate::node::item_status::ItemStatus<'_>,
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    println!();

    print_time_spent(menu_for, do_now_list);

    if let Some(finished_at) = menu_for.get_item().get_finished_at().as_ref() {
        let x_local = finished_at.with_timezone(&chrono::Local);
        println!("Finished: {}", x_local.format("%a %d %b %Y %I:%M%p"));
    } else {
        println!("Finished: (unknown)");
    }

    println!("Selected Item:");
    println!(
        "{}",
        DisplayItemNode::new(
            menu_for.get_item_node(),
            Filter::Active,
            DisplayFormat::MultiLineTree
        )
    );

    print_children(menu_for);

    println!();

    let choices = vec![
        FinishedItemDetailChoice::BackToFinishedList,
        FinishedItemDetailChoice::Reactivate,
    ];

    let selection = Select::new("Select an action...", choices)
        .with_page_size(default_select_page_size())
        .prompt();

    match selection {
        Ok(FinishedItemDetailChoice::Reactivate) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::ReactivateItem {
                    item: menu_for.get_surreal_record_id().clone(),
                })
                .await
                .unwrap();

            println!("Item reactivated.");
            Box::pin(present_finished_items_menu(send_to_data_storage_layer)).await
        }
        Ok(FinishedItemDetailChoice::BackToFinishedList) | Err(InquireError::OperationCanceled) => {
            Box::pin(present_finished_items_menu(send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

fn print_time_spent(menu_for: &crate::node::item_status::ItemStatus<'_>, do_now_list: &DoNowList) {
    print!("Time Spent: ");
    let items = vec![menu_for.get_item()];
    let now = do_now_list.get_now();
    let time_spent = do_now_list
        .get_time_spent_log()
        .iter()
        .filter(|x| x.did_work_towards_any(&items))
        .collect::<Vec<_>>();

    if time_spent.is_empty() {
        println!("None");
        return;
    }

    println!();

    let a_day_ago = *now - chrono::Duration::days(1);
    let last_day = time_spent
        .iter()
        .filter(|x| x.is_within(&a_day_ago, now))
        .fold((chrono::Duration::default(), 0), |acc, x| {
            (acc.0 + x.get_time_delta(), acc.1 + 1)
        });

    let a_week_ago = *now - chrono::Duration::weeks(1);
    let last_week = time_spent
        .iter()
        .filter(|x| x.is_within(&a_week_ago, now))
        .fold((chrono::Duration::default(), 0), |acc, x| {
            (acc.0 + x.get_time_delta(), acc.1 + 1)
        });

    let a_month_ago = *now - chrono::Duration::weeks(4);
    let last_month = time_spent
        .iter()
        .filter(|x| x.is_within(&a_month_ago, now))
        .fold((chrono::Duration::default(), 0), |acc, x| {
            (acc.0 + x.get_time_delta(), acc.1 + 1)
        });

    let total = time_spent
        .iter()
        .fold((chrono::Duration::default(), 0), |acc, x| {
            (acc.0 + x.get_time_delta(), acc.1 + 1)
        });

    if last_day.1 != total.1 {
        print!("    Last Day: ");
        if last_day.1 == 0 {
            println!("None");
        } else {
            println!(
                "{} times for {}",
                last_day.1,
                DisplayDuration::new(&last_day.0.to_std().expect("Can convert"))
            );
        }
    }

    if last_week.1 != last_day.1 {
        print!("    Last Week: ");
        if last_week.1 == 0 {
            println!("None");
        } else {
            println!(
                "{} times for {}",
                last_week.1,
                DisplayDuration::new(&last_week.0.to_std().expect("Can convert"))
            );
        }
    }

    if last_month.1 != last_week.1 {
        print!("    Last Month: ");
        if last_month.1 == 0 {
            println!("None");
        } else {
            println!(
                "{} times for {}",
                last_month.1,
                DisplayDuration::new(&last_month.0.to_std().expect("Can convert"))
            );
        }
    }

    println!(
        "    TOTAL: {} times for {}",
        total.1,
        DisplayDuration::new(&total.0.to_std().expect("Can convert"))
    );
    println!();
}

fn print_children(menu_for: &crate::node::item_status::ItemStatus<'_>) {
    let mut completed_children = menu_for
        .get_children(Filter::Finished)
        .map(|x| x.get_item())
        .collect::<Vec<_>>();
    completed_children.sort_by(|a, b| a.get_finished_at().cmp(b.get_finished_at()));

    if !completed_children.is_empty() {
        println!("Completed Actions:");
        for child in completed_children.iter().take(8) {
            println!("  ✅{}", DisplayItem::new(child));
        }
        if completed_children.len() > 8 {
            println!("  {} more ✅", completed_children.len() - 8);
        }
    }

    let in_progress_children = menu_for
        .get_children(Filter::Active)
        .map(|x| x.get_item())
        .collect::<Vec<_>>();
    if !in_progress_children.is_empty() {
        println!("Smaller Actions:");
        for child in in_progress_children {
            println!("  {}", DisplayItem::new(child));
        }
    }
}
