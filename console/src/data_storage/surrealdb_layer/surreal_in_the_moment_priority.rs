use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use surrealdb::{
    opt::RecordId,
    sql::{Datetime, Thing},
};

use super::SurrealTrigger;

//derive Builder is only for tests, I tried adding it just for cfg_attr(test... but that
//gave me false errors in the editor (rust-analyzer) so I am just going to try including
//it always to see if that addresses these phantom errors. Nov2023.
#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Builder)]
pub(crate) struct SurrealInTheMomentPriority {
    pub(crate) id: Option<Thing>,
    pub(crate) choice: SurrealAction,
    pub(crate) kind: SurrealPriorityKind,
    pub(crate) not_chosen: Vec<SurrealAction>,
    pub(crate) in_effect_until: Vec<SurrealTrigger>,

    #[cfg_attr(test, builder(default))]
    pub(crate) created: Datetime,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealAction {
    SetReadyAndUrgency(RecordId),
    ParentBackToAMotivation(RecordId),
    ReviewItem(RecordId),
    ItemNeedsAClassification(RecordId),
    PickItemReviewFrequency(RecordId),
    MakeProgress(RecordId),
    StateIfInMode { item: RecordId, mode: RecordId },
}

impl SurrealAction {
    pub(crate) fn get_record_id(&self) -> &RecordId {
        match self {
            SurrealAction::SetReadyAndUrgency(record_id)
            | SurrealAction::ParentBackToAMotivation(record_id)
            | SurrealAction::ReviewItem(record_id)
            | SurrealAction::ItemNeedsAClassification(record_id)
            | SurrealAction::PickItemReviewFrequency(record_id)
            | SurrealAction::MakeProgress(record_id) => record_id,
            SurrealAction::StateIfInMode { item, .. } => item,
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
pub(crate) enum SurrealPriorityKind {
    HighestPriority,
    LowestPriority,
}

impl SurrealInTheMomentPriority {
    pub(crate) const TABLE_NAME: &'static str = "in_the_moment_priorities";
}
