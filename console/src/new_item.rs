use chrono::{DateTime, Utc};
use derive_builder::Builder;
use surrealdb::sql::Datetime;

use crate::{
    data_storage::surrealdb_layer::surreal_item::{
        Responsibility, SurrealDependency, SurrealFrequency, SurrealItemType, SurrealLap,
        SurrealReviewGuidance, SurrealUrgencyPlan,
    },
    new_event::NewEvent,
};

#[derive(Builder, Clone, Debug)]
#[builder(setter(into))]
pub(crate) struct NewItem {
    pub(crate) summary: String,

    #[builder(default)]
    pub(crate) finished: Option<Datetime>,

    #[builder(default)]
    pub(crate) responsibility: Responsibility,

    #[builder(default)]
    pub(crate) item_type: SurrealItemType,

    #[builder(default = "Utc::now()")]
    pub(crate) created: DateTime<Utc>,

    #[builder(default)]
    pub(crate) urgency_plan: Option<SurrealUrgencyPlan>,

    #[builder(default)]
    pub(crate) lap: Option<SurrealLap>,

    #[builder(default)]
    pub(crate) dependencies: Vec<NewDependency>,

    #[builder(default)]
    pub(crate) last_reviewed: Option<DateTime<Utc>>,

    #[builder(default)]
    pub(crate) review_frequency: Option<SurrealFrequency>,

    #[builder(default)]
    pub(crate) review_guidance: Option<SurrealReviewGuidance>,
}

/// This type exists because it is possible to add a new event to a new item meaning that both need to be created at the same time.
#[derive(Clone, Debug)]
pub(crate) enum NewDependency {
    /// Dependency already exists in the database.
    Existing(SurrealDependency),
    /// Dependency is a new event that needs to be created.
    NewEvent(NewEvent),
}

impl NewItem {
    pub(crate) fn new(summary: String, now: DateTime<Utc>) -> Self {
        NewItem {
            summary,
            finished: None,
            responsibility: Responsibility::default(),
            item_type: SurrealItemType::Undeclared,
            created: now,
            urgency_plan: None,
            lap: None,
            dependencies: Default::default(),
            last_reviewed: None,
            review_frequency: None,
            review_guidance: None,
        }
    }
}
