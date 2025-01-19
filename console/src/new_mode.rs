use derive_builder::Builder;
use surrealdb::sql::Thing;

use crate::data_storage::surrealdb_layer::surreal_mode::SurrealScope;

#[derive(Builder)]
#[builder(setter(into))]
pub(crate) struct NewMode {
    pub(crate) summary: String,

    #[builder(default)]
    pub(crate) parent_mode: Option<Thing>,

    #[builder(default)]
    pub(crate) core_in_scope: Vec<SurrealScope>,

    #[builder(default)]
    pub(crate) non_core_in_scope: Vec<SurrealScope>,

    #[builder(default)]
    pub(crate) explicitly_out_of_scope_items: Vec<Thing>,
}
