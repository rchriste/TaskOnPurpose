use chrono::Utc;
use inquire::{InquireError, Select};
use tokio::sync::mpsc::Sender;

use crate::{
    base_data::{BaseData, item::Item},
    data_storage::surrealdb_layer::data_layer_commands::DataLayerCommands,
    display::display_item::DisplayItem,
};

use crate::menu::inquire::default_select_page_size;

use super::ItemTypeSelection;

pub(crate) async fn something_else_should_be_done_first(
    unable_to_do: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let surreal_tables = DataLayerCommands::get_raw_data(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
    let list = base_data
        .get_active_items()
        .iter()
        .copied()
        .map(DisplayItem::new)
        .collect::<Vec<_>>();
    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(should_be_done_first) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::CoverItemWithAnExistingItem {
                    item_to_be_covered: unable_to_do.get_surreal_record_id().clone(),
                    item_that_should_do_the_covering: should_be_done_first
                        .get_surreal_record_id()
                        .clone(),
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::OperationCanceled | InquireError::InvalidConfiguration(_)) => {
            something_else_should_be_done_first_new_item(unable_to_do, send_to_data_storage_layer)
                .await
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) async fn something_else_should_be_done_first_new_item(
    unable_to_do: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = ItemTypeSelection::create_list();
    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ItemTypeSelection::NormalHelp) => {
            ItemTypeSelection::print_normal_help();
            Box::pin(something_else_should_be_done_first_new_item(
                unable_to_do,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(selection) => {
            let new_item = selection.create_new_item_prompt_user_for_summary();
            send_to_data_storage_layer
                .send(DataLayerCommands::CoverItemWithANewItem {
                    cover_this: unable_to_do.get_surreal_record_id().clone(),
                    cover_with: new_item,
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(_) => todo!(),
    }
}
