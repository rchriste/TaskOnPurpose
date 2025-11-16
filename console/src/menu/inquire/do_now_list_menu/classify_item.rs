use tokio::sync::mpsc::Sender;

use crate::{
    data_storage::surrealdb_layer::data_layer_commands::DataLayerCommands,
    node::item_status::ItemStatus,
};

use super::do_now_list_single_item::declare_item_type;

pub(crate) async fn present_item_needs_a_classification_menu(
    item_status: &ItemStatus<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    declare_item_type(item_status.get_item(), send_to_data_storage_layer).await?;

    Ok(())
}
