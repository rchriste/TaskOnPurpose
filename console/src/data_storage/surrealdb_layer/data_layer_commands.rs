use chrono::Utc;
use std::{future::Future, pin::Pin};
use surrealdb::{
    Error as SurrealError, RecordId, Surreal,
    engine::any::{Any, IntoEndpoint, connect},
    err::Error as CoreError,
    kvs::Datastore,
    opt::PatchOp,
    opt::auth,
    sql::Datetime,
};
use tokio::{
    join,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot::{self, error::RecvError},
    },
};

use crate::{
    data_storage::surrealdb_layer::surreal_mode::SurrealMode,
    new_event::NewEvent,
    new_item::{NewDependency, NewItem},
    new_mode::NewMode,
    new_time_spent::NewTimeSpent,
};

use futures::stream::{self, StreamExt};

use super::{
    SurrealTrigger,
    surreal_current_mode::{NewCurrentMode, SurrealCurrentMode},
    surreal_event::SurrealEvent,
    surreal_in_the_moment_priority::{
        SurrealAction, SurrealInTheMomentPriority, SurrealPriorityKind,
    },
    surreal_item::{
        Responsibility, SurrealDependency, SurrealFrequency, SurrealItem, SurrealItemOldVersion,
        SurrealItemType, SurrealOrderedSubItem, SurrealReviewGuidance, SurrealUrgencyPlan,
    },
    surreal_mode,
    surreal_tables::SurrealTables,
    surreal_time_spent::{SurrealTimeSpent, SurrealTimeSpentVersion0},
    surreal_working_on::SurrealWorkingOn,
};

pub(crate) enum DataLayerCommands {
    SendRawData(oneshot::Sender<SurrealTables>),
    SendTimeSpentLog(oneshot::Sender<Vec<SurrealTimeSpent>>),
    RecordTimeSpent(NewTimeSpent),
    SetWorkingOn {
        item: RecordId,
        when_started: Datetime,
    },
    ClearWorkingOn,
    FinishItem {
        item: RecordId,
        when_finished: Datetime,
    },
    ReactivateItem {
        item: RecordId,
    },
    NewItem(NewItem),
    NewMode(NewMode),
    CoverItemWithANewItem {
        cover_this: RecordId,
        cover_with: NewItem,
    },
    CoverItemWithAnExistingItem {
        item_to_be_covered: RecordId,
        item_that_should_do_the_covering: RecordId,
    },
    UpdateRelativeImportance {
        parent: RecordId,
        update_this_child: RecordId,
        higher_importance_than_this_child: Option<RecordId>,
    },
    ParentItemWithExistingItem {
        child: RecordId,
        parent: RecordId,
        higher_importance_than_this: Option<RecordId>,
    },
    ParentItemWithANewChildItem {
        child: NewItem,
        parent: RecordId,
        higher_importance_than_this: Option<RecordId>,
    },
    ParentNewItemWithAnExistingChildItem {
        child: RecordId,
        parent_new_item: NewItem,
    },
    ParentItemRemoveParent {
        child: RecordId,
        parent_to_remove: RecordId,
    },
    UpdateResponsibilityAndItemType(RecordId, Responsibility, SurrealItemType),
    AddItemDependency(RecordId, SurrealDependency),
    RemoveItemDependency(RecordId, SurrealDependency),
    AddItemDependencyNewEvent(RecordId, NewEvent),
    UpdateSummary(RecordId, String),
    UpdateModeName(RecordId, String),
    UpdateUrgencyPlan(RecordId, Option<SurrealUrgencyPlan>),
    UpdateItemReviewFrequency(RecordId, SurrealFrequency, SurrealReviewGuidance),
    UpdateItemLastReviewedDate(RecordId, Datetime),
    DeclareInTheMomentPriority {
        choice: SurrealAction,
        kind: SurrealPriorityKind,
        not_chosen: Vec<SurrealAction>,
        in_effect_until: Vec<SurrealTrigger>,
    },
    ClearInTheMomentPriority(RecordId),
    SetCurrentMode(NewCurrentMode),
    TriggerEvent {
        event: RecordId,
        when: Datetime,
    },
    UntriggerEvent {
        event: RecordId,
        when: Datetime,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CopyDestinationBehavior {
    ErrorIfNotEmpty,
    ForceDeleteExisting,
}

type DeleteFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

fn box_delete_future<'a, Fut>(future: Fut) -> DeleteFuture<'a>
where
    Fut: Future<Output = Result<(), String>> + Send + 'a,
{
    Box::pin(future)
}

/// Generic helper function to delete a record from the database.
async fn delete_record<T>(db: &Surreal<Any>, id: &RecordId, type_name: &str) -> Result<(), String>
where
    T: serde::de::DeserializeOwned,
{
    let deleted: Option<T> = db
        .delete(id)
        .await
        .map_err(|e| format!("Failed to delete {} {:?}: {e:?}", type_name, id))?;
    deleted.ok_or_else(|| format!("{} {:?} not found for deletion", type_name, id))?;
    Ok(())
}

/// Helper function to create a stream of deletion futures for a collection of records.
fn create_delete_stream<'a, T, R>(
    db: &'a Surreal<Any>,
    records: Vec<R>,
    type_name: &'static str,
) -> impl futures::Stream<Item = DeleteFuture<'a>>
where
    T: serde::de::DeserializeOwned + 'a,
    R: Into<Option<RecordId>> + 'a,
{
    stream::iter(records).map(move |record| {
        let id_opt: Option<RecordId> = record.into();
        box_delete_future(async move {
            if let Some(id) = id_opt {
                delete_record::<T>(db, &id, type_name).await?;
            }
            Ok(())
        })
    })
}

#[derive(Clone, Debug)]
pub(crate) struct SurrealDbConnectionConfig {
    pub(crate) endpoint: String,
    pub(crate) namespace: String,
    pub(crate) database: String,
    pub(crate) auth: Option<SurrealAuthConfig>,
}

#[derive(Clone, Debug)]
pub(crate) struct SurrealAuthConfig {
    pub(crate) username: String,
    pub(crate) password: String,
    /// "root" | "ns" | "db" (defaults to "root")
    pub(crate) level: Option<String>,
}

impl DataLayerCommands {
    pub(crate) async fn get_raw_data(
        sender: &Sender<DataLayerCommands>,
    ) -> Result<SurrealTables, RecvError> {
        let (raw_data_sender, raw_data_receiver) = oneshot::channel();
        sender
            .send(DataLayerCommands::SendRawData(raw_data_sender))
            .await
            .unwrap();
        raw_data_receiver.await
    }
}

pub(crate) async fn data_storage_start_and_run(
    mut data_storage_layer_receive_rx: Receiver<DataLayerCommands>,
    config: SurrealDbConnectionConfig,
) {
    let endpoint = config.endpoint.clone();
    let db = match connect(endpoint.as_str()).await {
        Ok(db) => db,
        Err(err) => {
            // If the stored data on disk is from an older SurrealDB storage format,
            // automatically apply SurrealDB's built-in storage fixes and retry.
            if matches!(err, SurrealError::Db(CoreError::OutdatedStorageVersion)) {
                println!(
                    "Detected outdated SurrealDB storage version at '{}'. Upgrading storage in place...",
                    endpoint
                );
                if let Err(upgrade_err) = upgrade_surreal_storage(endpoint.as_str()).await {
                    panic!(
                        "Failed to upgrade SurrealDB storage automatically: {:?}",
                        upgrade_err
                    );
                }
                connect(endpoint.as_str()).await.unwrap()
            } else {
                panic!("Failed to connect to SurrealDB: {:?}", err);
            }
        }
    };

    if let Some(auth_cfg) = &config.auth
        && let Err(err) = authenticate_surrealdb(&db, &config, auth_cfg).await
    {
        eprintln!(
            "Failed to authenticate to SurrealDB (endpoint='{}'): {:?}\n\
             If this is a remote SurrealDB, validate your login credentials and consider passing:\n\
             - --surreal-auth-username <user>\n\
             - --surreal-auth-password <pass>\n\
             - --surreal-auth-level root|ns|db\n",
            config.endpoint, err
        );
        return;
    }

    ensure_namespace_and_migrate_if_needed(&db, &config).await;

    // let updated: Option<SurrealItem> = db.update((SurrealItem::TABLE_NAME, "5i5mkemqn0f1716v3ycw"))
    //     .patch(PatchOp::replace("/urgency_plan", None::<Option<SurrealUrgencyPlan>>)).await.unwrap();
    // assert!(updated.is_some());
    // panic!("Finished");
    loop {
        let received = data_storage_layer_receive_rx.recv().await;
        match received {
            Some(DataLayerCommands::SendRawData(oneshot)) => {
                let surreal_tables = load_from_surrealdb_upgrade_if_needed(&db).await;
                oneshot.send(surreal_tables).unwrap();
            }
            Some(DataLayerCommands::SendTimeSpentLog(sender)) => send_time_spent(sender, &db).await,
            Some(DataLayerCommands::RecordTimeSpent(new_time_spent)) => {
                record_time_spent(new_time_spent, &db).await
            }
            Some(DataLayerCommands::SetWorkingOn { item, when_started }) => {
                set_working_on(item, when_started, &db).await
            }
            Some(DataLayerCommands::ClearWorkingOn) => clear_working_on(&db).await,
            Some(DataLayerCommands::FinishItem {
                item,
                when_finished,
            }) => finish_item(item, when_finished, &db).await,
            Some(DataLayerCommands::ReactivateItem { item }) => reactivate_item(item, &db).await,
            Some(DataLayerCommands::NewItem(new_item)) => {
                create_new_item(new_item, &db).await;
            }
            Some(DataLayerCommands::CoverItemWithANewItem {
                cover_this,
                cover_with,
            }) => cover_with_a_new_item(cover_this, cover_with, &db).await,
            Some(DataLayerCommands::CoverItemWithAnExistingItem {
                item_to_be_covered,
                item_that_should_do_the_covering,
            }) => {
                cover_item_with_an_existing_item(
                    item_to_be_covered,
                    item_that_should_do_the_covering,
                    &db,
                )
                .await
            }
            Some(DataLayerCommands::NewMode(new_mode)) => {
                let mut surreal_mode: SurrealMode = new_mode.into();
                let created: SurrealMode = db
                    .create(surreal_mode::SurrealMode::TABLE_NAME)
                    .content(surreal_mode.clone())
                    .await
                    .unwrap()
                    .expect("Created");

                surreal_mode.id = created.id.clone();
                assert_eq!(surreal_mode, created);
            }
            Some(DataLayerCommands::ParentItemWithExistingItem {
                child,
                parent,
                higher_importance_than_this,
            }) => {
                parent_item_with_existing_item(child, parent, higher_importance_than_this, &db)
                    .await
            }
            Some(DataLayerCommands::ParentItemWithANewChildItem {
                child,
                parent,
                higher_importance_than_this,
            }) => {
                parent_item_with_a_new_child(child, parent, higher_importance_than_this, &db).await
            }
            Some(DataLayerCommands::ParentNewItemWithAnExistingChildItem {
                child,
                parent_new_item,
            }) => parent_new_item_with_an_existing_child_item(child, parent_new_item, &db).await,
            Some(DataLayerCommands::ParentItemRemoveParent {
                child,
                parent_to_remove,
            }) => {
                let mut parent: SurrealItem =
                    db.select(parent_to_remove.clone()).await.unwrap().unwrap();

                parent.smaller_items_in_priority_order = parent
                    .smaller_items_in_priority_order
                    .into_iter()
                    .filter(|x| match x {
                        SurrealOrderedSubItem::SubItem { surreal_item_id } => {
                            surreal_item_id != &child
                        }
                    })
                    .collect::<Vec<_>>();
                let saved = db
                    .update(&parent_to_remove)
                    .content(parent.clone())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(parent, saved);
            }
            Some(DataLayerCommands::AddItemDependency(record_id, new_ready)) => {
                add_dependency(record_id, new_ready, &db).await
            }
            Some(DataLayerCommands::RemoveItemDependency(record_id, to_remove)) => {
                remove_dependency(record_id, to_remove, &db).await
            }
            Some(DataLayerCommands::AddItemDependencyNewEvent(record_id, new_event)) => {
                add_dependency_new_event(record_id, new_event, &db).await
            }
            Some(DataLayerCommands::UpdateRelativeImportance {
                parent,
                update_this_child,
                higher_importance_than_this_child,
            }) => {
                parent_item_with_existing_item(
                    update_this_child,
                    parent,
                    higher_importance_than_this_child,
                    &db,
                )
                .await
            }
            Some(DataLayerCommands::UpdateItemLastReviewedDate(record_id, new_last_reviewed)) => {
                //TODO: I should probably fix this so it does the update all as one transaction rather than reading in the data and then changing it and writing it out again. That could cause issues if there are multiple writers. The reason why I didn't do it yet is because I only want to update part of the SurrealItemReview type and I need to experiment with the PatchOp::replace to see if and how to make it work with the nested type. Otherwise I might consider just making review_frequency and last_reviewed separate fields and then I can just update the review_frequency and not have to worry about the last_reviewed field.
                let mut item: SurrealItem = db.select(record_id.clone()).await.unwrap().unwrap();

                item.last_reviewed = Some(new_last_reviewed);
                let updated = db
                    .update(&record_id)
                    .content(item.clone())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(item, updated);
            }
            Some(DataLayerCommands::UpdateItemReviewFrequency(
                record_id,
                surreal_frequency,
                surreal_review_guidance,
            )) => {
                //TODO: I should probably fix this so it does the update all as one transaction rather than reading in the data and then changing it and writing it out again. That could cause issues if there are multiple writers. The reason why I didn't do it yet is because I only want to update part of the SurrealItemReview type and I need to experiment with the PatchOp::replace to see if and how to make it work with the nested type. Otherwise I might consider just making review_frequency and last_reviewed separate fields and then I can just update the review_frequency and not have to worry about the last_reviewed field.
                let previous_value: SurrealItem =
                    db.select(record_id.clone()).await.unwrap().unwrap();
                let mut item = previous_value.clone();
                item.review_frequency = Some(surreal_frequency);
                item.review_guidance = Some(surreal_review_guidance);
                let updated: SurrealItem = db
                    .update(&record_id)
                    .content(item.clone())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(item, updated);
            }
            Some(DataLayerCommands::UpdateSummary(item, new_summary)) => {
                update_item_summary(item, new_summary, &db).await
            }
            Some(DataLayerCommands::UpdateModeName(thing, new_name)) => {
                let updated: SurrealMode = db
                    .update(&thing)
                    .patch(PatchOp::replace("/name", new_name.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.name, new_name);
            }
            Some(DataLayerCommands::UpdateResponsibilityAndItemType(
                item,
                new_responsibility,
                new_item_type,
            )) => {
                let updated: SurrealItem = db
                    .update(&item)
                    .patch(PatchOp::replace(
                        "/responsibility",
                        new_responsibility.clone(),
                    ))
                    .patch(PatchOp::replace("/item_type", new_item_type.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.responsibility, new_responsibility);
                assert_eq!(updated.item_type, new_item_type);
            }
            Some(DataLayerCommands::UpdateUrgencyPlan(record_id, new_urgency_plan)) => {
                let updated: SurrealItem = db
                    .update(&record_id)
                    .patch(PatchOp::replace("/urgency_plan", new_urgency_plan.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.urgency_plan, new_urgency_plan);
            }
            Some(DataLayerCommands::DeclareInTheMomentPriority {
                choice,
                kind,
                not_chosen,
                in_effect_until,
            }) => {
                let mut priority = SurrealInTheMomentPriority {
                    id: None,
                    not_chosen,
                    in_effect_until,
                    created: Utc::now().into(),
                    choice,
                    kind,
                };
                let updated = db
                    .create(SurrealInTheMomentPriority::TABLE_NAME)
                    .content(priority.clone())
                    .await
                    .unwrap();
                let updated: SurrealInTheMomentPriority = updated.expect("Created");
                priority.id = updated.id.clone();
                assert_eq!(priority, updated);
            }
            Some(DataLayerCommands::ClearInTheMomentPriority(record_id)) => {
                let updated: SurrealInTheMomentPriority =
                    db.delete(&record_id).await.unwrap().unwrap();
                assert_eq!(updated.id, Some(record_id));
            }
            Some(DataLayerCommands::SetCurrentMode(new_current_mode)) => {
                let current_mode: SurrealCurrentMode = new_current_mode.into();
                let mut updated: Vec<SurrealCurrentMode> = db
                    .upsert(SurrealCurrentMode::TABLE_NAME)
                    .content(current_mode.clone())
                    .await
                    .unwrap();
                if updated.is_empty() {
                    //Annoyingly SurrealDB's upsert seems to just not work sometimes without giving an explicit error so I have to do this
                    updated = db
                        .insert(SurrealCurrentMode::TABLE_NAME)
                        .content(current_mode.clone())
                        .await
                        .unwrap();
                }
                assert_eq!(1, updated.len());
                let updated = updated.into_iter().next().unwrap();
                assert_eq!(current_mode, updated);
            }
            Some(DataLayerCommands::TriggerEvent { event, when }) => {
                let updated: SurrealEvent = db
                    .update(&event)
                    .patch(PatchOp::replace("/triggered", true))
                    .patch(PatchOp::replace("/last_updated", when.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.id, Some(event));
                assert!(updated.triggered);
                assert_eq!(updated.last_updated, when);
            }
            Some(DataLayerCommands::UntriggerEvent { event, when }) => {
                let updated: SurrealEvent = db
                    .update(&event)
                    .patch(PatchOp::replace("/triggered", false))
                    .patch(PatchOp::replace("/last_updated", when.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.id, Some(event));
                assert!(!updated.triggered);
                assert_eq!(updated.last_updated, when);
            }
            None => return, //Channel closed, time to shutdown down, exit
        }
    }
}

async fn authenticate_surrealdb(
    db: &Surreal<Any>,
    conn: &SurrealDbConnectionConfig,
    auth_cfg: &SurrealAuthConfig,
) -> Result<(), SurrealError> {
    let level = auth_cfg
        .level
        .as_deref()
        .unwrap_or("root")
        .to_ascii_lowercase();

    match level.as_str() {
        "root" => {
            db.signin(auth::Root {
                username: auth_cfg.username.as_str(),
                password: auth_cfg.password.as_str(),
            })
            .await?;
        }
        "ns" | "namespace" => {
            db.signin(auth::Namespace {
                namespace: conn.namespace.as_str(),
                username: auth_cfg.username.as_str(),
                password: auth_cfg.password.as_str(),
            })
            .await?;
        }
        "db" | "database" => {
            db.signin(auth::Database {
                namespace: conn.namespace.as_str(),
                database: conn.database.as_str(),
                username: auth_cfg.username.as_str(),
                password: auth_cfg.password.as_str(),
            })
            .await?;
        }
        other => {
            // Default to root auth if they provided an unknown value
            println!(
                "Unknown --surreal-auth-level '{}'; defaulting to 'root'.",
                other
            );
            db.signin(auth::Root {
                username: auth_cfg.username.as_str(),
                password: auth_cfg.password.as_str(),
            })
            .await?;
        }
    }

    Ok(())
}

fn surreal_tables_has_any_data(tables: &SurrealTables) -> bool {
    !tables.surreal_items.is_empty()
        || !tables.surreal_time_spent_log.is_empty()
        || !tables.surreal_in_the_moment_priorities.is_empty()
        || !tables.surreal_current_modes.is_empty()
        || !tables.surreal_modes.is_empty()
        || !tables.surreal_events.is_empty()
}

fn auth_configs_equivalent(a: &Option<SurrealAuthConfig>, b: &Option<SurrealAuthConfig>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.username == b.username && a.password == b.password && a.level == b.level
        }
        _ => false,
    }
}

async fn connect_with_auto_upgrade(endpoint: &str) -> Result<Surreal<Any>, String> {
    match connect(endpoint).await {
        Ok(db) => Ok(db),
        Err(err) => {
            if matches!(err, SurrealError::Db(CoreError::OutdatedStorageVersion)) {
                eprintln!(
                    "Detected outdated SurrealDB storage version at '{}'. Upgrading storage in place...",
                    endpoint
                );
                upgrade_surreal_storage(endpoint)
                    .await
                    .map_err(|upgrade_err| {
                        format!(
                            "Failed to upgrade SurrealDB storage automatically: {:?}",
                            upgrade_err
                        )
                    })?;
                connect(endpoint).await.map_err(|retry_err| {
                    format!(
                        "Failed to connect to SurrealDB after upgrade: {:?}",
                        retry_err
                    )
                })
            } else {
                Err(format!("Failed to connect to SurrealDB: {:?}", err))
            }
        }
    }
}

async fn connect_and_prepare(config: &SurrealDbConnectionConfig) -> Result<Surreal<Any>, String> {
    let db = connect_with_auto_upgrade(config.endpoint.as_str()).await?;
    if let Some(auth_cfg) = &config.auth {
        authenticate_surrealdb(&db, config, auth_cfg)
            .await
            .map_err(|err| {
                format!(
                    "Failed to authenticate to SurrealDB (endpoint='{}'): {:?}",
                    config.endpoint, err
                )
            })?;
    }
    Ok(db)
}

/// Trait for types that can be copied to the database preserving their IDs.
/// This trait is used to abstract over different SurrealDB table types.
trait SurrealTable: serde::Serialize + serde::de::DeserializeOwned + Clone + Send + std::fmt::Debug + 'static {
    /// The name of the table in the database
    const TABLE_NAME: &'static str;
    
    /// Get the record ID if it exists
    fn id(&self) -> &Option<RecordId>;
    
    /// Get a human-readable type name for error messages
    fn type_name() -> &'static str;
}

impl SurrealTable for SurrealItem {
    const TABLE_NAME: &'static str = SurrealItem::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealItem" }
}

impl SurrealTable for SurrealTimeSpent {
    const TABLE_NAME: &'static str = SurrealTimeSpent::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealTimeSpent" }
}

impl SurrealTable for SurrealInTheMomentPriority {
    const TABLE_NAME: &'static str = SurrealInTheMomentPriority::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealInTheMomentPriority" }
}

impl SurrealTable for SurrealCurrentMode {
    const TABLE_NAME: &'static str = SurrealCurrentMode::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealCurrentMode" }
}

impl SurrealTable for SurrealMode {
    const TABLE_NAME: &'static str = surreal_mode::SurrealMode::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealMode" }
}

impl SurrealTable for SurrealEvent {
    const TABLE_NAME: &'static str = SurrealEvent::TABLE_NAME;
    fn id(&self) -> &Option<RecordId> { &self.id }
    fn type_name() -> &'static str { "SurrealEvent" }
}

/// Generic function to copy records to the database preserving their IDs.
/// This function handles the common pattern of upserting records with a fallback to insert.
async fn copy_records_preserving_ids<T>(
    db: &Surreal<Any>,
    records: Vec<T>,
) -> Result<(), String>
where
    T: SurrealTable,
{
    stream::iter(records.into_iter()) // wrap in a stream so we can use buffer_unordered below to limit concurrency
        .map(|record| async move {
            let mut updated: Vec<T> = db
                .upsert(T::TABLE_NAME)
                .content(record.clone())
                .await
                .map_err(|e| format!("Failed to upsert {}: {e:?}", T::type_name()))?;
            if updated.is_empty() {
                // Same workaround as elsewhere in this file: upsert can silently do nothing.
                updated = db
                    .insert(T::TABLE_NAME)
                    .content(record.clone())
                    .await
                    .map_err(|e| format!("Failed to insert {}: {e:?}", T::type_name()))?;
            }
            if updated.is_empty() {
                return Err(format!("Failed to copy {} {:?}", T::type_name(), record.id()));
            }

            Ok(())
        })
        .buffer_unordered(100) //limit concurrency
        .fold(Ok(()), |acc, res| async {
            match (acc, res) {
                (Ok(_), Ok(())) => Ok(()),
                (Err(e), _) | (Ok(()), Err(e)) => Err(e), //Propagate first error encountered
            }
        })
        .await?;

    Ok(())
}

async fn copy_surreal_items_preserving_ids(
    db: &Surreal<Any>,
    surreal_items: Vec<SurrealItem>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_items).await
}

async fn copy_surreal_time_spent_preserving_ids(
    db: &Surreal<Any>,
    surreal_time_spent_log: Vec<SurrealTimeSpent>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_time_spent_log).await
}

async fn copy_surreal_in_the_moment_priorities_preserving_ids(
    db: &Surreal<Any>,
    surreal_in_the_moment_priorities: Vec<SurrealInTheMomentPriority>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_in_the_moment_priorities).await
}

async fn copy_surreal_current_modes_preserving_ids(
    db: &Surreal<Any>,
    surreal_current_modes: Vec<SurrealCurrentMode>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_current_modes).await
}

async fn copy_surreal_modes_preserving_ids(
    db: &Surreal<Any>,
    surreal_modes: Vec<SurrealMode>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_modes).await
}

async fn copy_surreal_events_preserving_ids(
    db: &Surreal<Any>,
    surreal_events: Vec<SurrealEvent>,
) -> Result<(), String> {
    copy_records_preserving_ids(db, surreal_events).await
}

async fn copy_surreal_tables_preserving_ids(
    db: &Surreal<Any>,
    tables: SurrealTables,
) -> Result<(), String> {
    // Copy records preserving record IDs so references remain valid.
    //Note that if a new table is added to the database then the below code needs to be updated to copy that table as well.
    let (items, time_spent, priorities, modes, events, current_mode) = join!(
        biased; // prefer earlier futures to run first as they should have more data
        copy_surreal_items_preserving_ids(db, tables.surreal_items),
        copy_surreal_time_spent_preserving_ids(db, tables.surreal_time_spent_log),
        copy_surreal_in_the_moment_priorities_preserving_ids(
            db,
            tables.surreal_in_the_moment_priorities,
        ),
        copy_surreal_modes_preserving_ids(db, tables.surreal_modes),
        copy_surreal_events_preserving_ids(db, tables.surreal_events),
        copy_surreal_current_modes_preserving_ids(db, tables.surreal_current_modes),
    );

    // The `?` error propagation operator can't be used inside the join! macro, so apply it here.
    items?;
    time_spent?;
    priorities?;
    current_mode?;
    modes?;
    events?;

    Ok(())
}

async fn copy_between_databases_if_destination_empty_same_connection(
    db: &Surreal<Any>,
    source_ns: &str,
    source_db: &str,
    destination_ns: &str,
    destination_db: &str,
    behavior: CopyDestinationBehavior,
) -> Result<(), String> {
    db.use_ns(destination_ns)
        .use_db(destination_db)
        .await
        .map_err(|e| format!("Failed to use destination ns/db: {e:?}"))?;
    let destination_tables = load_from_surrealdb_upgrade_if_needed(db).await;
    if surreal_tables_has_any_data(&destination_tables) {
        match behavior {
            CopyDestinationBehavior::ForceDeleteExisting => {
                clear_surreal_tables(db, destination_tables).await?;
            }
            CopyDestinationBehavior::ErrorIfNotEmpty => {
                return Err(format!(
                    "Destination database is not empty (ns='{}' db='{}'). Use --force to delete existing data before copying.",
                    destination_ns, destination_db
                ));
            }
        }
    }

    db.use_ns(source_ns)
        .use_db(source_db)
        .await
        .map_err(|e| format!("Failed to use source ns/db: {e:?}"))?;
    let source_tables = load_from_surrealdb_upgrade_if_needed(db).await;

    db.use_ns(destination_ns)
        .use_db(destination_db)
        .await
        .map_err(|e| format!("Failed to re-select destination ns/db: {e:?}"))?;

    copy_surreal_tables_preserving_ids(db, source_tables).await
}

async fn clear_surreal_tables(db: &Surreal<Any>, tables: SurrealTables) -> Result<(), String> {
    create_delete_stream::<SurrealItem, _>(db, tables.surreal_items, SurrealItem::TABLE_NAME)
        .chain(create_delete_stream::<SurrealTimeSpent, _>(
            db,
            tables.surreal_time_spent_log,
            SurrealTimeSpent::TABLE_NAME,
        ))
        .chain(create_delete_stream::<SurrealInTheMomentPriority, _>(
            db,
            tables.surreal_in_the_moment_priorities,
            SurrealInTheMomentPriority::TABLE_NAME,
        ))
        .chain(create_delete_stream::<SurrealCurrentMode, _>(
            db,
            tables.surreal_current_modes,
            SurrealCurrentMode::TABLE_NAME,
        ))
        .chain(create_delete_stream::<SurrealMode, _>(
            db,
            tables.surreal_modes,
            SurrealMode::TABLE_NAME,
        ))
        .chain(create_delete_stream::<SurrealEvent, _>(
            db,
            tables.surreal_events,
            SurrealEvent::TABLE_NAME,
        ))
        .buffer_unordered(100)
        .fold(Ok(()), |acc, res| async {
            match (acc, res) {
                (Ok(_), Ok(())) => Ok(()),
                (Err(e), _) | (Ok(()), Err(e)) => Err(e),
            }
        })
        .await?;

    Ok(())
}

pub(crate) async fn copy_database_if_destination_empty(
    source: SurrealDbConnectionConfig,
    destination: SurrealDbConnectionConfig,
    behavior: CopyDestinationBehavior,
) -> Result<(), String> {
    if source.endpoint == destination.endpoint {
        if !auth_configs_equivalent(&source.auth, &destination.auth) {
            return Err(
                "Source/destination auth configs differ for the same endpoint; this command currently requires them to match."
                    .to_string(),
            );
        }

        let db = connect_and_prepare(&destination).await?;
        copy_between_databases_if_destination_empty_same_connection(
            &db,
            source.namespace.as_str(),
            source.database.as_str(),
            destination.namespace.as_str(),
            destination.database.as_str(),
            behavior,
        )
        .await
    } else {
        let source_db = connect_and_prepare(&source).await?;
        source_db
            .use_ns(source.namespace.as_str())
            .use_db(source.database.as_str())
            .await
            .map_err(|e| format!("Failed to select source ns/db: {e:?}"))?;
        let source_tables = load_from_surrealdb_upgrade_if_needed(&source_db).await;

        let dest_db = connect_and_prepare(&destination).await?;
        dest_db
            .use_ns(destination.namespace.as_str())
            .use_db(destination.database.as_str())
            .await
            .map_err(|e| format!("Failed to select destination ns/db: {e:?}"))?;
        let dest_tables = load_from_surrealdb_upgrade_if_needed(&dest_db).await;
        if surreal_tables_has_any_data(&dest_tables) {
            match behavior {
                CopyDestinationBehavior::ForceDeleteExisting => {
                    clear_surreal_tables(&dest_db, dest_tables).await?;
                }
                CopyDestinationBehavior::ErrorIfNotEmpty => {
                    return Err(format!(
                        "Destination database is not empty (endpoint='{}' ns='{}' db='{}'). Use --force to delete existing data before copying.",
                        destination.endpoint, destination.namespace, destination.database
                    ));
                }
            }
        }

        copy_surreal_tables_preserving_ids(&dest_db, source_tables).await
    }
}

async fn ensure_namespace_and_migrate_if_needed(
    db: &Surreal<Any>,
    config: &SurrealDbConnectionConfig,
) {
    let target_ns = config.namespace.as_str();
    let db_name = config.database.as_str();

    // Always use target namespace/db going forward.
    db.use_ns(target_ns).use_db(db_name).await.unwrap();

    // If the target namespace already has data, nothing to do.
    let target_tables = load_from_surrealdb_upgrade_if_needed(db).await;
    if surreal_tables_has_any_data(&target_tables) {
        return;
    }

    // Legacy fallback: copy from `OnPurpose` if it has data.
    // Old data may have been stored under the hardcoded db name "Russ", or under the current username.
    let legacy_ns = "OnPurpose";
    let legacy_db_candidates = if db_name == "Russ" {
        vec![db_name]
    } else {
        vec![db_name, "Russ"]
    };

    let mut legacy_tables: Option<SurrealTables> = None;
    let mut legacy_db_used: Option<&str> = None;
    for legacy_db in legacy_db_candidates {
        db.use_ns(legacy_ns).use_db(legacy_db).await.unwrap();
        let tables = load_from_surrealdb_upgrade_if_needed(db).await;
        if surreal_tables_has_any_data(&tables) {
            legacy_tables = Some(tables);
            legacy_db_used = Some(legacy_db);
            break;
        }
    }

    let Some(legacy_tables) = legacy_tables else {
        // Restore target context.
        db.use_ns(target_ns).use_db(db_name).await.unwrap();
        return;
    };

    println!(
        "No data found in SurrealDB namespace '{}'. Copying existing data from legacy namespace '{}' (db='{}') into '{}' (db='{}')...",
        target_ns,
        legacy_ns,
        legacy_db_used.unwrap_or("UNKNOWN"),
        target_ns,
        db_name
    );

    db.use_ns(target_ns).use_db(db_name).await.unwrap();

    copy_surreal_tables_preserving_ids(db, legacy_tables)
        .await
        .unwrap();

    println!(
        "SurrealDB namespace copy complete. Continuing with namespace '{}' (db='{}').",
        target_ns, db_name
    );
}

/// Upgrade an embedded/local SurrealDB datastore to the latest storage version.
/// This uses SurrealDB-core's `Version::fix` to apply any required on-disk migrations.
async fn upgrade_surreal_storage(endpoint: &str) -> Result<(), CoreError> {
    #[allow(deprecated)]
    let ep = endpoint.into_endpoint().map_err(|e| match e {
        SurrealError::Db(db) => db,
        other => CoreError::Ds(other.to_string()),
    })?;

    // Match SurrealDB embedded engine behavior: TiKV uses full URL, others use parsed path.
    let ds_endpoint = if ep.url.scheme() == "tikv" {
        ep.url.as_str().to_owned()
    } else {
        ep.path.clone()
    };

    let ds = Datastore::new(&ds_endpoint).await?;
    let ds = std::sync::Arc::new(ds);
    let version = ds.get_version().await?;
    if version.is_latest() {
        return Ok(());
    }
    version.fix(ds).await?;
    Ok(())
}

pub(crate) async fn load_from_surrealdb_upgrade_if_needed(db: &Surreal<Any>) -> SurrealTables {
    //TODO: I should do some timings to see if starting all of these get_all requests and then doing awaits on them later really is faster in Rust. Or if they just for sure don't start until the await. For example I could call this function as many times as possible in 10 sec and time that and then see how many times I can call that function written like this and then again with the get_all being right with the await to make sure that code like this is worth it perf wise.
    let all_items = db.select(SurrealItem::TABLE_NAME);
    let time_spent_log = db.select(SurrealTimeSpent::TABLE_NAME);
    let surreal_in_the_moment_priorities = db.select(SurrealInTheMomentPriority::TABLE_NAME);
    let surreal_current_modes = db.select(SurrealCurrentMode::TABLE_NAME);
    let surreal_modes = db.select(surreal_mode::SurrealMode::TABLE_NAME);
    let surreal_events = db.select(SurrealEvent::TABLE_NAME);
    let surreal_working_on = db.select(SurrealWorkingOn::TABLE_NAME);

    let all_items: Vec<SurrealItem> = match all_items.await {
        Ok(all_items) => {
            if all_items.iter().any(|x: &SurrealItem| x.version == 1) {
                upgrade_items_table_version1_to_version2(db).await;
                db.select(SurrealItem::TABLE_NAME).await.unwrap()
            } else {
                all_items
            }
        }
        Err(err) => {
            let err_string = err.to_string();
            if err_string.contains("IAM error")
                || err_string.contains("Not enough permissions")
                || err_string.contains("not enough permissions")
            {
                panic!(
                    "SurrealDB permissions error while reading items table: {}\n\
                     If connecting to a remote SurrealDB, you likely need to authenticate.\n\
                     Try: --surreal-auth-username <user> --surreal-auth-password <pass> [--surreal-auth-level root|ns|db]\n",
                    err_string
                );
            }
            println!("Upgrading items table because of issue: {}", err);
            upgrade_items_table(db).await;
            db.select(SurrealItem::TABLE_NAME).await.unwrap()
        }
    };

    let time_spent_log = match time_spent_log.await {
        Ok(time_spent_log) => time_spent_log,
        Err(err) => {
            println!("Time spent log is missing because of issue: {}", err);
            upgrade_time_spent_log(db).await;
            db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap()
        }
    };

    let surreal_in_the_moment_priorities = surreal_in_the_moment_priorities.await.unwrap();

    let surreal_modes = surreal_modes.await.unwrap();

    SurrealTables {
        surreal_items: all_items,
        surreal_time_spent_log: time_spent_log,
        surreal_in_the_moment_priorities,
        surreal_current_modes: surreal_current_modes.await.unwrap(),
        surreal_modes,
        surreal_events: surreal_events.await.unwrap(),
        surreal_working_on: surreal_working_on.await.unwrap(),
    }
}

async fn upgrade_items_table_version1_to_version2(db: &Surreal<Any>) {
    let a: Vec<SurrealItem> = db.select(SurrealItemOldVersion::TABLE_NAME).await.unwrap();
    for mut item_old_version in a.into_iter() {
        let item: SurrealItem =
            if matches!(item_old_version.item_type, SurrealItemType::Motivation(_)) {
                item_old_version.responsibility = Responsibility::ReactiveBeAvailableToAct;
                item_old_version.version = 2;
                item_old_version
            } else {
                item_old_version.version = 2;
                item_old_version
            };
        let item_record_id = item.id.clone().expect("In DB");
        let updated: SurrealItem = db
            .update(&item_record_id)
            .content(item.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(item, updated);
    }
}

async fn upgrade_items_table(db: &Surreal<Any>) {
    let a: Vec<SurrealItemOldVersion> = db.select(SurrealItemOldVersion::TABLE_NAME).await.unwrap();
    for item_old_version in a.into_iter() {
        let item: SurrealItem = item_old_version.into();
        let item_record_id = item.id.clone().expect("In DB");
        let updated: SurrealItem = db
            .update(&item_record_id)
            .content(item.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(item, updated);
    }
}

async fn upgrade_time_spent_log(db: &Surreal<Any>) {
    let a: Vec<SurrealTimeSpentVersion0> = db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
    for time_spent_old in a.into_iter() {
        let time_spent: SurrealTimeSpent = time_spent_old.into();
        let time_spent_record_id = time_spent.id.clone().expect("In DB");
        let updated: SurrealTimeSpent = db
            .update(&time_spent_record_id)
            .content(time_spent.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(time_spent, updated);
    }
}

async fn send_time_spent(sender: oneshot::Sender<Vec<SurrealTimeSpent>>, db: &Surreal<Any>) {
    let time_spent = db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
    sender.send(time_spent).unwrap();
}

async fn set_working_on(item: RecordId, when_started: Datetime, db: &Surreal<Any>) {
    let record = SurrealWorkingOn::new(item, when_started);
    let mut updated: Vec<SurrealWorkingOn> = db
        .upsert(SurrealWorkingOn::TABLE_NAME)
        .content(record.clone())
        .await
        .unwrap();
    if updated.is_empty() {
        // Annoyingly SurrealDB's upsert seems to just not work sometimes without giving an explicit
        // error so we fall back to insert.
        updated = db
            .insert(SurrealWorkingOn::TABLE_NAME)
            .content(record.clone())
            .await
            .unwrap();
    }
    assert!(!updated.is_empty());
}

async fn clear_working_on(db: &Surreal<Any>) {
    let id: RecordId = (SurrealWorkingOn::TABLE_NAME, "working_on").into();
    // Ignore if it doesn't exist.
    let _deleted: Option<SurrealWorkingOn> = db.delete(id).await.unwrap();
}

async fn record_time_spent(new_time_spent: NewTimeSpent, db: &Surreal<Any>) {
    let mut new_time_spent: SurrealTimeSpent = new_time_spent.into();
    let saved: SurrealTimeSpent = db
        .create(SurrealTimeSpent::TABLE_NAME)
        .content(new_time_spent.clone())
        .await
        .unwrap()
        .expect("Created");
    new_time_spent.id = saved.id.clone();
    assert_eq!(new_time_spent, saved);
}

pub(crate) async fn finish_item(finish_this: RecordId, when_finished: Datetime, db: &Surreal<Any>) {
    let updated: SurrealItem = db
        .update(&finish_this)
        .patch(PatchOp::replace("/finished", Some(when_finished.clone())))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.finished, Some(when_finished));
}

pub(crate) async fn reactivate_item(reactivate_this: RecordId, db: &Surreal<Any>) {
    let updated: SurrealItem = db
        .update(&reactivate_this)
        .patch(PatchOp::replace("/finished", None::<Datetime>))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.finished, None);
}

async fn create_new_item(mut new_item: NewItem, db: &Surreal<Any>) -> SurrealItem {
    for dependency in new_item.dependencies.iter_mut() {
        match dependency {
            NewDependency::NewEvent(new_event) => {
                let created = create_new_event(new_event.clone(), db).await;
                let created_event_record_id = created.id.expect("In DB");
                *dependency =
                    NewDependency::Existing(SurrealDependency::AfterEvent(created_event_record_id));
            }
            NewDependency::Existing(_) => {}
        }
    }
    let mut surreal_item: SurrealItem = SurrealItem::new(new_item, vec![])
        .expect("We fix up NewDependency::NewEvent above so it will never happen here");
    let created: SurrealItem = db
        .create(SurrealItem::TABLE_NAME)
        .content(surreal_item.clone())
        .await
        .unwrap()
        .expect("Created");
    surreal_item.id = created.id.clone();
    assert_eq!(surreal_item, created);

    created
}

async fn cover_with_a_new_item(cover_this: RecordId, cover_with: NewItem, db: &Surreal<Any>) {
    let cover_with = create_new_item(cover_with, db).await;

    let cover_with_record_id = cover_with.id.expect("In DB");
    let new_dependency = SurrealDependency::AfterItem(cover_with_record_id);
    add_dependency(cover_this, new_dependency, db).await;
}

async fn cover_item_with_an_existing_item(
    existing_item_to_be_covered: RecordId,
    existing_item_that_is_doing_the_covering: RecordId,
    db: &Surreal<Any>,
) {
    let new_dependency = SurrealDependency::AfterItem(existing_item_that_is_doing_the_covering);
    add_dependency(existing_item_to_be_covered, new_dependency, db).await;
}

async fn parent_item_with_existing_item(
    child_record_id: RecordId,
    parent_record_id: RecordId,
    higher_importance_than_this: Option<RecordId>,
    db: &Surreal<Any>,
) {
    //TODO: This should be refactored so it happens inside of a transaction and ideally as one query because if the data is modified between the time that the data is read and the time that the data is written back out then the data could be lost. I haven't done this yet because I need to figure out how to do this inside of a SurrealDB query and I haven't done that yet.
    let mut parent: SurrealItem = db.select(parent_record_id.clone()).await.unwrap().unwrap();
    parent.smaller_items_in_priority_order = parent
        .smaller_items_in_priority_order
        .into_iter()
        .filter(|x| match x {
            SurrealOrderedSubItem::SubItem { surreal_item_id } => {
                surreal_item_id != &child_record_id
            }
        })
        .collect::<Vec<_>>();
    if let Some(higher_priority_than_this) = higher_importance_than_this {
        let index_of_higher_priority = parent
            .smaller_items_in_priority_order
            .iter()
            .position(|x| match x {
                //Note that position() is short-circuiting. If there are multiple matches it could be argued that I should panic or assert but
                //I am just matching the first one and then I just keep going. Because I am still figuring out the design and this is
                //more in the vein of hardening work I think this is fine but feel free to revisit this.
                SurrealOrderedSubItem::SubItem { surreal_item_id } => {
                    surreal_item_id == &higher_priority_than_this
                }
            })
            .expect("Should already be in the list");
        parent.smaller_items_in_priority_order.insert(
            index_of_higher_priority,
            SurrealOrderedSubItem::SubItem {
                surreal_item_id: child_record_id,
            },
        );
    } else {
        parent
            .smaller_items_in_priority_order
            .push(SurrealOrderedSubItem::SubItem {
                surreal_item_id: child_record_id,
            });
    }
    let saved = db
        .update(&parent_record_id)
        .content(parent.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent, saved);
}

async fn parent_item_with_a_new_child(
    child: NewItem,
    parent: RecordId,
    higher_importance_than_this: Option<RecordId>,
    db: &Surreal<Any>,
) {
    let child = create_new_item(child, db).await;
    parent_item_with_existing_item(
        child.id.expect("In DB"),
        parent,
        higher_importance_than_this,
        db,
    )
    .await
}

async fn parent_new_item_with_an_existing_child_item(
    child: RecordId,
    mut parent_new_item: NewItem,
    db: &Surreal<Any>,
) {
    for dependency in parent_new_item.dependencies.iter_mut() {
        match dependency {
            NewDependency::NewEvent(new_event) => {
                let created = create_new_event(new_event.clone(), db).await;
                let created_event_record_id = created.id.expect("In DB");
                *dependency =
                    NewDependency::Existing(SurrealDependency::AfterEvent(created_event_record_id));
            }
            NewDependency::Existing(_) => {}
        }
    }

    //TODO: Write a Unit Test for this
    let smaller_items_in_priority_order = vec![SurrealOrderedSubItem::SubItem {
        surreal_item_id: child,
    }];

    let mut parent_surreal_item =
        SurrealItem::new(parent_new_item, smaller_items_in_priority_order)
            .expect("We deal with new events above so it will never happen here");
    let created: SurrealItem = db
        .create(SurrealItem::TABLE_NAME)
        .content(parent_surreal_item.clone())
        .await
        .unwrap()
        .expect("Created");
    parent_surreal_item.id = created.id.clone();
    assert_eq!(parent_surreal_item, created);
}

async fn add_dependency(record_id: RecordId, new_dependency: SurrealDependency, db: &Surreal<Any>) {
    //TODO: This should be refactored so it happens inside of a transaction and ideally as one query because if the data is modified between the time that the data is read and the time that the data is written back out then the data could be lost. I haven't done this yet because I need to figure out how to do this inside of a SurrealDB query and I haven't done that yet.

    let mut surreal_item: SurrealItem = db
        .select(record_id.clone())
        .await
        .unwrap()
        .expect("Record exists");
    if surreal_item.dependencies.contains(&new_dependency) {
        //Is already there, nothing to do
    } else {
        surreal_item.dependencies.push(new_dependency);

        let updated: SurrealItem = db
            .update(&record_id)
            .content(surreal_item.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(surreal_item, updated);
    }
}

async fn remove_dependency(record_id: RecordId, to_remove: SurrealDependency, db: &Surreal<Any>) {
    let mut surreal_item: SurrealItem = db.select(record_id.clone()).await.unwrap().unwrap();
    surreal_item.dependencies.retain(|x| x != &to_remove);

    let update = db
        .update(&record_id)
        .content(surreal_item.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(surreal_item, update);
}

async fn add_dependency_new_event(record_id: RecordId, new_event: NewEvent, db: &Surreal<Any>) {
    let created: SurrealEvent = create_new_event(new_event, db).await;
    let new_dependency = SurrealDependency::AfterEvent(created.id.expect("In DB"));

    add_dependency(record_id, new_dependency, db).await
}

async fn create_new_event(new_event: NewEvent, db: &Surreal<Any>) -> SurrealEvent {
    let event: SurrealEvent = new_event.into();
    let created: SurrealEvent = db
        .create(SurrealEvent::TABLE_NAME)
        .content(event.clone())
        .await
        .unwrap()
        .expect("Created");
    assert_eq!(created.last_updated, event.last_updated);
    assert_eq!(created.summary, event.summary);
    created
}

async fn update_item_summary(item_to_update: RecordId, new_summary: String, db: &Surreal<Any>) {
    let updated: SurrealItem = db
        .update(&item_to_update)
        .patch(PatchOp::replace("/summary", new_summary.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.summary, new_summary);
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    use crate::{
        data_storage::surrealdb_layer::surreal_item::SurrealHowMuchIsInMyControl,
        new_item::NewItemBuilder,
    };

    fn mem_config() -> SurrealDbConnectionConfig {
        SurrealDbConnectionConfig {
            endpoint: "mem://".to_string(),
            namespace: "TaskOnPurpose".to_string(),
            database: "test".to_string(),
            auth: None,
        }
    }

    #[tokio::test]
    async fn data_starts_empty() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert!(surreal_tables.surreal_items.is_empty());
        assert!(surreal_tables.surreal_time_spent_log.is_empty());

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn add_new_item() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let new_item = NewItem::new("New item".into(), Utc::now());
        sender
            .send(DataLayerCommands::NewItem(new_item))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(surreal_tables.surreal_items.len(), 1);
        assert_eq!(
            SurrealItemType::Undeclared,
            surreal_tables.surreal_items.first().unwrap().item_type
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn finish_item() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let new_next_step = NewItemBuilder::default()
            .summary("New next step")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(new_next_step))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);

        assert_eq!(items.len(), 1);
        let next_step_item = items.iter().next().map(|(_, v)| v).unwrap();
        assert!(!next_step_item.is_finished());

        let when_finished = Utc::now();
        sender
            .send(DataLayerCommands::FinishItem {
                item: next_step_item.get_surreal_record_id().clone(),
                when_finished: when_finished.into(),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);

        assert_eq!(items.len(), 1);
        let next_step_item = items.iter().next().map(|(_, v)| v).unwrap();
        assert!(next_step_item.is_finished());

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn reactivate_item_after_finish_clears_finished_field() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let new_next_step = NewItemBuilder::default()
            .summary("New next step")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(new_next_step))
            .await
            .unwrap();

        // Capture the record id so we can reference it across reloads.
        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let item_id: RecordId = items
            .iter()
            .next()
            .map(|(_, v)| v.get_surreal_record_id().clone())
            .expect("item exists");

        // Finish it
        let when_finished = Utc::now();
        sender
            .send(DataLayerCommands::FinishItem {
                item: item_id.clone(),
                when_finished: when_finished.into(),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        assert_eq!(surreal_tables.surreal_items.len(), 1);
        assert!(
            surreal_tables
                .surreal_items
                .first()
                .expect("exists")
                .finished
                .is_some()
        ); //Make sure that the test case or scenario is valid

        // Reactivate it
        sender
            .send(DataLayerCommands::ReactivateItem {
                item: item_id.clone(),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        assert_eq!(surreal_tables.surreal_items.len(), 1);
        assert!(
            surreal_tables
                .surreal_items
                .first()
                .expect("exists")
                .finished
                .is_none()
        );

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let item = items.iter().next().map(|(_, v)| v).unwrap();
        assert!(!item.is_finished());
        assert!(item.get_finished_at().is_none());

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn cover_item_with_a_new_proactive_next_step() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let new_action = NewItemBuilder::default()
            .summary("Item to be covered")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(new_action))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(1, surreal_tables.surreal_items.len());
        assert_eq!(
            0,
            surreal_tables
                .surreal_items
                .first()
                .unwrap()
                .dependencies
                .len()
        ); //length of zero means nothing is covered
        let item_to_cover = surreal_tables.surreal_items.first().unwrap();

        let new_item = NewItemBuilder::default()
            .summary("Covering item")
            .responsibility(Responsibility::ProactiveActionToTake)
            .item_type(SurrealItemType::Action)
            .build()
            .unwrap();

        sender
            .send(DataLayerCommands::CoverItemWithANewItem {
                cover_this: item_to_cover.id.clone().expect("In DB"),
                cover_with: new_item,
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        let item_that_should_be_covered = surreal_tables
            .surreal_items
            .iter()
            .find(|x| x.summary == "Item to be covered")
            .unwrap();
        assert_eq!(2, surreal_tables.surreal_items.len());
        assert_eq!(1, item_that_should_be_covered.dependencies.len()); //expect one item to be is covered
        let item_that_should_cover = surreal_tables
            .surreal_items
            .iter()
            .find(|x| x.summary == "Covering item")
            .unwrap();
        let id = match &item_that_should_be_covered.dependencies.first().unwrap() {
            SurrealDependency::AfterItem(id) => id,
            _ => panic!("Should be an item"),
        };
        assert_eq!(item_that_should_cover.id.as_ref().unwrap(), id,);

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn parent_item_with_a_new_item() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let new_action = NewItemBuilder::default()
            .summary("Item that needs a parent")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(new_action))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(1, surreal_tables.surreal_items.len());

        sender
            .send(DataLayerCommands::ParentNewItemWithAnExistingChildItem {
                child: surreal_tables
                    .surreal_items
                    .into_iter()
                    .next()
                    .unwrap()
                    .id
                    .expect("In Db"),
                parent_new_item: NewItemBuilder::default()
                    .summary("Parent Item")
                    .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default()))
                    .build()
                    .unwrap(),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(2, surreal_tables.surreal_items.len());
        assert_eq!(
            1,
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
                .len()
        );
        assert_eq!(
            &SurrealOrderedSubItem::SubItem {
                surreal_item_id: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Item that needs a parent")
                    .unwrap()
                    .id
                    .as_ref()
                    .unwrap()
                    .clone()
            },
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
                .first()
                .unwrap()
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn parent_item_with_an_existing_item_that_has_no_children() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let item_that_needs_a_parent = NewItemBuilder::default()
            .summary("Item that needs a parent")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(item_that_needs_a_parent))
            .await
            .unwrap();

        let parent_item = NewItemBuilder::default()
            .summary("Parent Item")
            .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default()))
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(parent_item))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(2, surreal_tables.surreal_items.len());

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Item that needs a parent")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: None,
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(2, surreal_tables.surreal_items.len());
        assert_eq!(
            1,
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
                .len()
        );
        assert_eq!(
            &SurrealOrderedSubItem::SubItem {
                surreal_item_id: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Item that needs a parent")
                    .unwrap()
                    .id
                    .as_ref()
                    .unwrap()
                    .clone()
            },
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
                .first()
                .unwrap()
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn parent_item_with_an_existing_item_that_has_children() {
        // SETUP
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let child_item = NewItemBuilder::default()
            .summary("Child Item at the top of the list")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item 2nd position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item 3rd position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item bottom position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let parent_item = NewItemBuilder::default()
            .summary("Parent Item")
            .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default()))
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(parent_item))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(5, surreal_tables.surreal_items.len());

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item at the top of the list")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: None,
            })
            .await
            .unwrap();

        // TEST - The order of adding the items is meant to cause the higher_priority_than_this to be used

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item bottom position")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: None,
            })
            .await
            .unwrap();

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item 2nd position")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: Some(
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .clone()
                        .expect("In DB"),
                ),
            })
            .await
            .unwrap();

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item 3rd position")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: Some(
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .clone()
                        .expect("In DB"),
                ),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(
            vec![
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item at the top of the list")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 3rd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
            ],
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn change_order_of_children() {
        // SETUP
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        let child_item = NewItemBuilder::default()
            .summary("Child Item at the top of the list")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item 2nd position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item 3rd position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let child_item = NewItemBuilder::default()
            .summary("Child Item bottom position, then moved to above 2nd position")
            .item_type(SurrealItemType::Action)
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(child_item))
            .await
            .unwrap();

        let parent_item = NewItemBuilder::default()
            .summary("Parent Item")
            .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default()))
            .build()
            .expect("Filled out required fields");
        sender
            .send(DataLayerCommands::NewItem(parent_item))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(5, surreal_tables.surreal_items.len());

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item at the top of the list")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: None,
            })
            .await
            .unwrap();

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| {
                        x.summary == "Child Item bottom position, then moved to above 2nd position"
                    })
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: None,
            })
            .await
            .unwrap();

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item 2nd position")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: Some(
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| {
                            x.summary
                                == "Child Item bottom position, then moved to above 2nd position"
                        })
                        .unwrap()
                        .id
                        .clone()
                        .expect("In DB"),
                ),
            })
            .await
            .unwrap();

        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Child Item 3rd position")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                higher_importance_than_this: Some(
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| {
                            x.summary
                                == "Child Item bottom position, then moved to above 2nd position"
                        })
                        .unwrap()
                        .id
                        .clone()
                        .expect("In DB"),
                ),
            })
            .await
            .unwrap();

        // TEST - Move the bottom item to the 2nd position
        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| {
                        x.summary == "Child Item bottom position, then moved to above 2nd position"
                    })
                    .unwrap()
                    .id
                    .clone()
                    .expect("In DB"),
                parent: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Parent Item")
                    .unwrap()
                    .id
                    .as_ref()
                    .expect("In DB")
                    .clone(),
                higher_importance_than_this: Some(
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .clone()
                        .expect("In DB"),
                ),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(
            vec![
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item at the top of the list")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary
                            == "Child Item bottom position, then moved to above 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 3rd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone()
                },
            ],
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_priority_order
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn copy_between_databases_when_destination_empty_copies_data() {
        let db = connect("mem://").await.unwrap();

        // Seed source database.
        db.use_ns("TaskOnPurpose")
            .use_db("copy_source")
            .await
            .unwrap();
        create_new_item(NewItem::new("Seed item".into(), Utc::now()), &db).await;

        // Copy into empty destination database.
        copy_between_databases_if_destination_empty_same_connection(
            &db,
            "TaskOnPurpose",
            "copy_source",
            "TaskOnPurpose",
            "copy_dest",
            CopyDestinationBehavior::ErrorIfNotEmpty,
        )
        .await
        .unwrap();

        db.use_ns("TaskOnPurpose")
            .use_db("copy_dest")
            .await
            .unwrap();
        let tables = load_from_surrealdb_upgrade_if_needed(&db).await;
        assert!(surreal_tables_has_any_data(&tables));
        assert_eq!(tables.surreal_items.len(), 1);
    }

    #[tokio::test]
    async fn copy_between_databases_when_destination_not_empty_errors() {
        let db = connect("mem://").await.unwrap();

        // Seed source database.
        db.use_ns("TaskOnPurpose")
            .use_db("copy_source2")
            .await
            .unwrap();
        create_new_item(NewItem::new("Seed item".into(), Utc::now()), &db).await;

        // Seed destination database so it is NOT empty.
        db.use_ns("TaskOnPurpose")
            .use_db("copy_dest2")
            .await
            .unwrap();
        create_new_item(NewItem::new("Existing dest item".into(), Utc::now()), &db).await;

        let err = copy_between_databases_if_destination_empty_same_connection(
            &db,
            "TaskOnPurpose",
            "copy_source2",
            "TaskOnPurpose",
            "copy_dest2",
            CopyDestinationBehavior::ErrorIfNotEmpty,
        )
        .await
        .expect_err("Destination is not empty so copy should refuse");

        assert!(err.contains("Destination database is not empty"));
    }
}
