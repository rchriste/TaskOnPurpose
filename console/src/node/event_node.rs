use ahash::HashMap;
use surrealdb::RecordId;

use crate::{
    base_data::event::Event,
    node::{
        Filter,
        item_status::{DependencyWithItemNode, ItemStatus},
    },
};

#[derive(Debug)]
pub(crate) struct EventNode<'s> {
    event: &'s Event<'s>,
    waiting_on_this: Vec<&'s ItemStatus<'s>>,
}

impl<'s> EventNode<'s> {
    pub(crate) fn new(
        event: &'s Event<'s>,
        all_items_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
    ) -> Self {
        let waiting_on_this = all_items_status
            .values()
            .filter(|item_status| {
                item_status.is_active()
                    && item_status
                        .get_dependencies(Filter::Active)
                        .any(|dependency| {
                            matches!(
                                dependency,
                                DependencyWithItemNode::AfterEvent(waiting_on_event)
                                    if waiting_on_event.get_surreal_record_id() == event.get_surreal_record_id()
                            )
                        })
            })
            .collect::<Vec<_>>();

        Self {
            event,
            waiting_on_this,
        }
    }

    pub(crate) fn get_waiting_on_this(&self) -> &[&'s ItemStatus<'s>] {
        &self.waiting_on_this
    }

    /// An event is "active" for UI purposes when it hasn't been triggered yet
    /// and there is at least one active item waiting on it.
    pub(crate) fn is_active(&self) -> bool {
        !self.event.is_triggered() && !self.waiting_on_this.is_empty()
    }

    pub(crate) fn get_summary(&self) -> &str {
        self.event.get_summary()
    }

    pub(crate) fn get_last_updated(&self) -> &chrono::DateTime<chrono::Utc> {
        self.event.get_last_updated()
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.event.get_surreal_record_id()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use surrealdb::RecordId;

    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            surreal_event::SurrealEvent,
            surreal_item::{SurrealDependency, SurrealItemBuilder, SurrealItemType},
            surreal_tables::SurrealTablesBuilder,
        },
    };

    #[test]
    fn event_node_is_inactive_when_event_is_triggered() {
        let now = Utc::now();
        let event_id: RecordId = ("events", "1").into();

        let surreal_event = SurrealEvent {
            id: Some(event_id.clone()),
            version: 0,
            last_updated: now.into(),
            triggered: true,
            summary: "Triggered event".to_string(),
        };

        let active_item_waiting_on_event = SurrealItemBuilder::default()
            .id(Some(("item", "1").into()))
            .summary("Active item waiting")
            .item_type(SurrealItemType::Action)
            .dependencies(vec![SurrealDependency::AfterEvent(event_id.clone())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![active_item_waiting_on_event])
            .surreal_events(vec![surreal_event])
            .build()
            .expect("no required fields");

        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        let event_node = calculated_data
            .get_event_nodes()
            .get(&event_id)
            .expect("event node exists");

        assert!(!event_node.is_active());
    }

    #[test]
    fn event_node_is_inactive_when_no_active_items_wait_on_it() {
        let now = Utc::now();
        let event_id: RecordId = ("events", "1").into();

        let surreal_event = SurrealEvent {
            id: Some(event_id.clone()),
            version: 0,
            last_updated: now.into(),
            triggered: false,
            summary: "Untriggered event".to_string(),
        };

        let unrelated_active_item = SurrealItemBuilder::default()
            .id(Some(("item", "1").into()))
            .summary("Active item not waiting")
            .item_type(SurrealItemType::Action)
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![unrelated_active_item])
            .surreal_events(vec![surreal_event])
            .build()
            .expect("no required fields");

        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        let event_node = calculated_data
            .get_event_nodes()
            .get(&event_id)
            .expect("event node exists");
        assert!(event_node.get_waiting_on_this().is_empty());
        assert!(!event_node.is_active());
    }

    #[test]
    fn event_node_is_active_when_untriggered_and_has_active_waiting_item() {
        let now = Utc::now();
        let event_id: RecordId = ("events", "1").into();

        let surreal_event = SurrealEvent {
            id: Some(event_id.clone()),
            version: 0,
            last_updated: now.into(),
            triggered: false,
            summary: "Untriggered event".to_string(),
        };

        let active_item_waiting_on_event = SurrealItemBuilder::default()
            .id(Some(("item", "1").into()))
            .summary("Active item waiting")
            .item_type(SurrealItemType::Action)
            .dependencies(vec![SurrealDependency::AfterEvent(event_id.clone())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![active_item_waiting_on_event])
            .surreal_events(vec![surreal_event])
            .build()
            .expect("no required fields");

        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        let event_node = calculated_data
            .get_event_nodes()
            .get(&event_id)
            .expect("event node exists");
        assert_eq!(event_node.get_waiting_on_this().len(), 1);
        assert!(event_node.is_active());
    }
}
