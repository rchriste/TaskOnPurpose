use derive_builder::Builder;
use surrealdb::types::RecordId;

#[derive(Builder)]
#[builder(setter(into))]
pub(crate) struct NewMode {
    pub(crate) name: String,

    #[builder(default)]
    pub(crate) parent: Option<RecordId>,
}
