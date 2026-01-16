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
    display::display_item_node::{DisplayFormat, DisplayItemNode},
    menu::inquire::{default_select_page_size, item_children_summary, time_spent_summary},
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
                x_local.format("%a %d %b %Y %I:%M%P").to_string()
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

    time_spent_summary::print_time_spent(menu_for, do_now_list);

    if let Some(finished_at) = menu_for.get_item().get_finished_at().as_ref() {
        let x_local = finished_at.with_timezone(&chrono::Local);
        println!("Finished: {}", x_local.format("%a %d %b %Y %I:%M%P"));
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

    item_children_summary::print_completed_children(menu_for);
    item_children_summary::print_in_progress_children(menu_for, do_now_list.get_all_items_status());

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
