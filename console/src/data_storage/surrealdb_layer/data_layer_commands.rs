use chrono::Utc;
use surrealdb::{
    Surreal,
    engine::any::{Any, IntoEndpoint, connect},
    opt::{PatchOp, RecordId},
    sql::{Datetime, Thing},
};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot::{self, error::RecvError},
};

use crate::{
    data_storage::surrealdb_layer::{
        surreal_mode::SurrealMode,
        surreal_time_spent::SurrealTimeSpentVersion1,
    },
    new_event::NewEvent,
    new_item::{NewDependency, NewItem},
    new_mode::NewMode,
    new_time_spent::NewTimeSpent,
};

use super::{
    SurrealTrigger,
    surreal_current_mode::{NewCurrentMode, SurrealCurrentMode},
    surreal_event::SurrealEvent,
    surreal_in_the_moment_priority::{
        SurrealAction, SurrealInTheMomentPriority, SurrealPriorityKind,
    },
    surreal_item::{
        Responsibility, SurrealDependency, SurrealFrequency, SurrealImportance, SurrealItem,
        SurrealItemType, SurrealModeScope, SurrealReviewGuidance, SurrealUrgencyPlan,
    },
    surreal_mode::{self, SurrealScope},
    surreal_tables::SurrealTables,
    surreal_time_spent::{SurrealTimeSpent, SurrealTimeSpentVersion0, SurrealTimeSpentVersionOnly},
};

pub(crate) enum DataLayerCommands {
    SendRawData(oneshot::Sender<SurrealTables>),
    SendTimeSpentLog(oneshot::Sender<Vec<SurrealTimeSpent>>),
    RecordTimeSpent(NewTimeSpent),
    FinishItem {
        item: RecordId,
        when_finished: Datetime,
    },
    NewItem(NewItem),
    NewMode(NewMode, oneshot::Sender<SurrealMode>),
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
        higher_importance_than_this_child: Option<(SurrealModeScope, Option<RecordId>)>,
    },
    ParentItemWithExistingItem {
        child: RecordId,
        parent: RecordId,
        higher_importance_than_this: Option<(SurrealModeScope, Option<RecordId>)>,
    },
    ParentItemWithANewChildItem {
        child: NewItem,
        parent: RecordId,
        higher_importance_than_this: Option<(SurrealModeScope, Option<RecordId>)>,
    },
    ParentNewItemWithAnExistingChildItem {
        child: RecordId,
        child_importance_scope: Option<SurrealModeScope>,
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
    UpdateModeSummary(RecordId, String),
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
    DeclareScopeForMode(ScopeModeCommand),
}

pub(crate) enum ScopeModeCommand {
    AddCore { mode: RecordId, scope: SurrealScope },
    RemoveCore { mode: RecordId, item: RecordId },
    AddNonCore { mode: RecordId, scope: SurrealScope },
    RemoveNonCore { mode: RecordId, item: RecordId },
    AddExplicitlyOutOfScope { mode: RecordId, item: RecordId },
    RemoveExplicitlyOutOfScope { mode: RecordId, item: RecordId },
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
    data_storage_layer_receive_rx: Receiver<DataLayerCommands>,
    endpoint: impl IntoEndpoint,
) {
    let db = data_storage_connect_to_db(endpoint).await;
    data_storage_endless_loop(db, data_storage_layer_receive_rx).await
}

pub(crate) async fn data_storage_connect_to_db(endpoint: impl IntoEndpoint) -> Surreal<Any> {
    let db = connect(endpoint).await.unwrap();
    db.use_ns("OnPurpose").use_db("Russ").await.unwrap(); //TODO: "Russ" should be a parameter, maybe the username or something
    db
}

pub(crate) async fn data_storage_endless_loop(
    db: Surreal<Any>,
    mut data_storage_layer_receive_rx: Receiver<DataLayerCommands>,
) {
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
            Some(DataLayerCommands::FinishItem {
                item,
                when_finished,
            }) => finish_item(item, when_finished, &db).await,
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
            Some(DataLayerCommands::NewMode(new_mode, record_id_of_new_item)) => {
                let mut surreal_mode: SurrealMode = new_mode.into();
                let created: SurrealMode = db
                    .create(surreal_mode::SurrealMode::TABLE_NAME)
                    .content(surreal_mode.clone())
                    .await
                    .unwrap()
                    .into_iter()
                    .next()
                    .unwrap();

                surreal_mode.id = created.id.clone();
                assert_eq!(surreal_mode, created);
                record_id_of_new_item.send(created).unwrap();
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
                child_importance_scope,
                parent_new_item,
            }) => {
                parent_new_item_with_an_existing_child_item(
                    child,
                    child_importance_scope,
                    parent_new_item,
                    &db,
                )
                .await
            }
            Some(DataLayerCommands::ParentItemRemoveParent {
                child,
                parent_to_remove,
            }) => {
                let mut parent: SurrealItem =
                    db.select(parent_to_remove.clone()).await.unwrap().unwrap();

                parent.smaller_items_in_importance_order = parent
                    .smaller_items_in_importance_order
                    .into_iter()
                    .filter(|x| x.child_item != child)
                    .collect::<Vec<_>>();
                parent.smaller_items_not_important = parent
                    .smaller_items_not_important
                    .into_iter()
                    .filter(|x| x != &child)
                    .collect::<Vec<_>>();

                let saved = db
                    .update(parent_to_remove)
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
                    .update(record_id)
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
                let updated = db
                    .update(record_id)
                    .content(item.clone())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(item, updated);
            }
            Some(DataLayerCommands::UpdateSummary(item, new_summary)) => {
                update_item_summary(item, new_summary, &db).await
            }
            Some(DataLayerCommands::UpdateModeSummary(thing, new_name)) => {
                let updated: SurrealMode = db
                    .update(thing)
                    .patch(PatchOp::replace("/summary", new_name.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.summary, new_name);
            }
            Some(DataLayerCommands::UpdateResponsibilityAndItemType(
                item,
                new_responsibility,
                new_item_type,
            )) => {
                let updated: SurrealItem = db
                    .update(item.clone())
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
                    .update(record_id)
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
                assert_eq!(1, updated.len());

                let updated: SurrealInTheMomentPriority = updated.into_iter().next().unwrap();
                priority.id = updated.id.clone();
                assert_eq!(priority, updated);
            }
            Some(DataLayerCommands::ClearInTheMomentPriority(record_id)) => {
                let updated: SurrealInTheMomentPriority = db
                    .delete((SurrealInTheMomentPriority::TABLE_NAME, record_id.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.id, Some(record_id));
            }
            Some(DataLayerCommands::SetCurrentMode(new_current_mode)) => {
                let current_mode: SurrealCurrentMode = new_current_mode.into();
                let mut updated = db
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
                    .update(event.clone())
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
                    .update(event.clone())
                    .patch(PatchOp::replace("/triggered", false))
                    .patch(PatchOp::replace("/last_updated", when.clone()))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(updated.id, Some(event));
                assert!(!updated.triggered);
                assert_eq!(updated.last_updated, when);
            }
            Some(DataLayerCommands::DeclareScopeForMode(command)) => match command {
                ScopeModeCommand::AddCore { mode, scope } => {
                    let updated: SurrealMode = db
                        .update(mode.clone())
                        .patch(PatchOp::add("/core_in_scope", vec![scope.clone()]))
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(updated.core_in_scope.contains(&scope));
                }
                ScopeModeCommand::RemoveCore { mode, item } => {
                    todo!()
                }
                ScopeModeCommand::AddNonCore { mode, scope } => {
                    let updated: SurrealMode = db
                        .update(mode.clone())
                        .patch(PatchOp::add("/non_core_in_scope", vec![scope.clone()]))
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(updated.non_core_in_scope.contains(&scope));
                }
                ScopeModeCommand::RemoveNonCore { mode, item } => {
                    todo!()
                }
                ScopeModeCommand::AddExplicitlyOutOfScope { mode, item } => {
                    let updated: SurrealMode = db
                        .update(mode.clone())
                        .patch(PatchOp::add(
                            "/explicitly_out_of_scope_items",
                            vec![item.clone()],
                        ))
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(updated.explicitly_out_of_scope_items.contains(&item));
                }
                ScopeModeCommand::RemoveExplicitlyOutOfScope { mode, item } => {
                    todo!()
                }
            },
            None => return, //Channel closed, time to shutdown down, exit
        }
    }
}

pub(crate) async fn load_from_surrealdb_upgrade_if_needed(db: &Surreal<Any>) -> SurrealTables {
    //TODO: I should do some timings to see if starting all of these get_all requests and then doing awaits on them later really is faster in Rust. Or if they just for sure don't start until the await. For example I could call this function as many times as possible in 10 sec and time that and then see how many times I can call that function written like this and then again with the get_all being right with the await to make sure that code like this is worth it perf wise.
    let all_items = db.select(SurrealItem::TABLE_NAME);
    let time_spent_log = db.select(SurrealTimeSpent::TABLE_NAME);
    let surreal_in_the_moment_priorities = db.select(SurrealInTheMomentPriority::TABLE_NAME);
    let surreal_current_modes = db.select(SurrealCurrentMode::TABLE_NAME);
    let surreal_modes = db.select(surreal_mode::SurrealMode::TABLE_NAME);
    let surreal_events = db.select(SurrealEvent::TABLE_NAME);

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

    let surreal_modes = match surreal_modes.await {
        Ok(surreal_modes) => surreal_modes,
        Err(err) => {
            println!("Surreal Modes is missing because of issue: {}", err);
            upgrade_modes_table(db).await;
            db.select(surreal_mode::SurrealMode::TABLE_NAME)
                .await
                .unwrap()
        }
    };

    SurrealTables {
        surreal_items: all_items,
        surreal_time_spent_log: time_spent_log,
        surreal_in_the_moment_priorities,
        surreal_current_modes: surreal_current_modes.await.unwrap(),
        surreal_modes,
        surreal_events: surreal_events.await.unwrap(),
    }
}

async fn upgrade_items_table_version1_to_version2(db: &Surreal<Any>) {
    todo!()
    // let a: Vec<SurrealItemOldVersion> = db.select(SurrealItemOldVersion::TABLE_NAME).await.unwrap();
    // for mut item_old_version in a.into_iter() {
    //     let item: SurrealItemVersion3 =
    //         if matches!(item_old_version.item_type, SurrealItemTypeVersion3::Motivation(_)) {
    //             item_old_version.responsibility = Responsibility::ReactiveBeAvailableToAct;
    //             item_old_version.version = 2;
    //             item_old_version
    //         } else {
    //             item_old_version.version = 2;
    //             item_old_version
    //         };
    //     let updated: SurrealItem = db
    //         .update(item.id.clone().unwrap())
    //         .content(item.clone())
    //         .await
    //         .unwrap()
    //         .unwrap();
    //     assert_eq!(item, updated);
    // }
}

async fn upgrade_items_table(db: &Surreal<Any>) {
    todo!()
    // let a: Vec<SurrealItemOldVersion> = db.select(SurrealItemOldVersion::TABLE_NAME).await.unwrap();
    // for item_old_version in a.into_iter() {
    //     let item: SurrealItem = item_old_version.into();
    //     let updated: SurrealItem = db
    //         .update(item.id.clone().unwrap())
    //         .content(item.clone())
    //         .await
    //         .unwrap()
    //         .unwrap();
    //     assert_eq!(item, updated);
    // }
}

async fn upgrade_time_spent_log(db: &Surreal<Any>) {
    let mut version_only: Vec<SurrealTimeSpentVersionOnly> =
        db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
    if version_only.iter().any(|x| x.version == 0) {
        let a: Vec<SurrealTimeSpentVersion0> =
            db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
        for time_spent_old in a.into_iter() {
            let time_spent: SurrealTimeSpentVersion1 = time_spent_old.into();
            let updated: SurrealTimeSpentVersion1 = db
                .update(time_spent.id.clone().expect("In DB"))
                .content(time_spent.clone())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(time_spent, updated);
        }
        version_only = db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
    }

    if version_only.iter().any(|x| x.version == 1) {
        let a: Vec<SurrealTimeSpentVersion1> =
            db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
        for time_spent_old in a.into_iter() {
            let time_spent: SurrealTimeSpent = time_spent_old.into();
            let updated: SurrealTimeSpent = db
                .update(time_spent.id.clone().expect("In DB"))
                .content(time_spent.clone())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(time_spent, updated);
        }
    }
}

async fn upgrade_modes_table(db: &Surreal<Any>) {
    let a: Vec<surreal_mode::SurrealModeVersion0> = db
        .select(surreal_mode::SurrealMode::TABLE_NAME)
        .await
        .unwrap();
    for mode_old_version in a.into_iter() {
        let mode: SurrealMode = mode_old_version.into();
        let updated: SurrealMode = db
            .update(mode.id.clone().expect("In DB"))
            .content(mode.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mode, updated);
    }
}

async fn send_time_spent(sender: oneshot::Sender<Vec<SurrealTimeSpent>>, db: &Surreal<Any>) {
    let time_spent = db.select(SurrealTimeSpent::TABLE_NAME).await.unwrap();
    sender.send(time_spent).unwrap();
}

async fn record_time_spent(new_time_spent: NewTimeSpent, db: &Surreal<Any>) {
    let mut new_time_spent: SurrealTimeSpent = new_time_spent.into();
    let saved: Vec<SurrealTimeSpent> = db
        .create(SurrealTimeSpent::TABLE_NAME)
        .content(new_time_spent.clone())
        .await
        .unwrap();
    assert_eq!(1, saved.len());
    let saved = saved.into_iter().next().unwrap();
    new_time_spent.id = saved.id.clone();
    assert_eq!(new_time_spent, saved);
}

pub(crate) async fn finish_item(finish_this: RecordId, when_finished: Datetime, db: &Surreal<Any>) {
    let updated: SurrealItem = db
        .update(finish_this)
        .patch(PatchOp::replace("/finished", Some(when_finished.clone())))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.finished, Some(when_finished));
}

async fn create_new_item(mut new_item: NewItem, db: &Surreal<Any>) -> SurrealItem {
    for dependency in new_item.dependencies.iter_mut() {
        match dependency {
            NewDependency::NewEvent(new_event) => {
                let created = create_new_event(new_event.clone(), db).await;
                *dependency = NewDependency::Existing(SurrealDependency::AfterEvent(
                    created.id.expect("In DB"),
                ));
            }
            NewDependency::Existing(_) => {}
        }
    }
    let mut surreal_item: SurrealItem = SurrealItem::new(new_item, vec![], vec![])
        .expect("We fix up NewDependency::NewEvent above so it will never happen here");
    let created: Vec<SurrealItem> = db
        .create(SurrealItem::TABLE_NAME)
        .content(surreal_item.clone())
        .await
        .unwrap();
    assert!(created.len() == 1);
    let created = created.into_iter().next().unwrap();
    surreal_item.id = created.id.clone();
    assert_eq!(surreal_item, created);

    created
}

async fn cover_with_a_new_item(cover_this: RecordId, cover_with: NewItem, db: &Surreal<Any>) {
    let cover_with = create_new_item(cover_with, db).await;

    let cover_with: Option<Thing> = cover_with.into();
    let cover_with = cover_with.expect("always exists the .into() wraps it in an option");
    let new_dependency = SurrealDependency::AfterItem(cover_with);
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

/// Some examples:
/// higher_importance_than_this: None -> Means the child_record should be marked as not important
/// higher_importance_than_this: Some((_, None)) -> Means the child_record should be as the lowest important on the list
async fn parent_item_with_existing_item(
    child_record_id: RecordId,
    parent_record_id: RecordId,
    higher_importance_than_this: Option<(SurrealModeScope, Option<RecordId>)>,
    db: &Surreal<Any>,
) {
    //TODO: This should be refactored so it happens inside of a transaction and ideally as one query because if the data is modified between the time that the data is read and the time that the data is written back out then the data could be lost. I haven't done this yet because I need to figure out how to do this inside of a SurrealDB query and I haven't done that yet.
    let mut parent: SurrealItem = db.select(parent_record_id.clone()).await.unwrap().unwrap();
    parent.smaller_items_in_importance_order = parent
        .smaller_items_in_importance_order
        .into_iter()
        .filter(|x| x.child_item != child_record_id)
        .collect::<Vec<_>>();
    parent.smaller_items_not_important = parent
        .smaller_items_not_important
        .into_iter()
        .filter(|x| x != &child_record_id)
        .collect::<Vec<_>>();

    match higher_importance_than_this {
        Some((mode_scope, Some(higher_priority_than_this))) => {
            let index_of_higher_priority = parent
                .smaller_items_in_importance_order
                .iter()
                .position(|x|
                //Note that position() is short-circuiting. If there are multiple matches it could be argued that I should panic or assert but
                //I am just matching the first one and then I just keep going. Because I am still figuring out the design and this is
                //more in the vein of hardening work I think this is fine but feel free to revisit this.
                x.child_item == higher_priority_than_this)
                .expect("Should already be in the list");
            parent.smaller_items_in_importance_order.insert(
                index_of_higher_priority,
                SurrealImportance {
                    child_item: child_record_id,
                    scope: mode_scope,
                },
            );
        }
        Some((mode_scope, None)) => {
            parent
                .smaller_items_in_importance_order
                .push(SurrealImportance {
                    child_item: child_record_id,
                    scope: mode_scope,
                });
        }
        None => {
            parent.smaller_items_not_important.push(child_record_id);
        }
    }

    let saved = db
        .update(parent_record_id)
        .content(parent.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent, saved);
}

async fn parent_item_with_a_new_child(
    child: NewItem,
    parent: RecordId,
    higher_importance_than_this: Option<(SurrealModeScope, Option<RecordId>)>,
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

/// if child_importance_scope is None then the child is not important
async fn parent_new_item_with_an_existing_child_item(
    child: RecordId,
    child_importance_scope: Option<SurrealModeScope>,
    mut parent_new_item: NewItem,
    db: &Surreal<Any>,
) {
    for dependency in parent_new_item.dependencies.iter_mut() {
        match dependency {
            NewDependency::NewEvent(new_event) => {
                let created = create_new_event(new_event.clone(), db).await;
                *dependency = NewDependency::Existing(SurrealDependency::AfterEvent(
                    created.id.expect("In DB"),
                ));
            }
            NewDependency::Existing(_) => {}
        }
    }

    //TODO: Write a Unit Test for this

    let mut parent_surreal_item = if let Some(child_mode_scope) = child_importance_scope {
        let smaller_items_in_priority_order = vec![SurrealImportance {
            child_item: child,
            scope: child_mode_scope,
        }];
        SurrealItem::new(parent_new_item, smaller_items_in_priority_order, vec![])
            .expect("We deal with new events above so it will never happen here")
    } else {
        SurrealItem::new(parent_new_item, vec![], vec![child])
            .expect("We deal with new events above so it will never happen here")
    };
    let created = db
        .create(SurrealItem::TABLE_NAME)
        .content(parent_surreal_item.clone())
        .await
        .unwrap();
    assert_eq!(1, created.len());
    let created: SurrealItem = created.into_iter().next().unwrap();
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
            .update(record_id)
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
        .update(record_id)
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
    let created = db
        .create(SurrealEvent::TABLE_NAME)
        .content(event.clone())
        .await
        .unwrap();
    assert_eq!(1, created.len());
    let created: SurrealEvent = created.into_iter().next().unwrap();
    assert_eq!(created.last_updated, event.last_updated);
    assert_eq!(created.summary, event.summary);
    created
}

async fn update_item_summary(item_to_update: RecordId, new_summary: String, db: &Surreal<Any>) {
    let updated: SurrealItem = db
        .update(item_to_update.clone())
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

    use crate::new_item::NewItemBuilder;

    #[tokio::test]
    async fn data_starts_empty() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
        assert_eq!(next_step_item.is_finished(), false);

        let when_finished = Utc::now();
        sender
            .send(DataLayerCommands::FinishItem {
                item: next_step_item.get_surreal_record_id().clone().into(),
                when_finished: when_finished.into(),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);

        assert_eq!(items.len(), 1);
        let next_step_item = items.iter().next().map(|(_, v)| v).unwrap();
        assert_eq!(next_step_item.is_finished(), true);

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn cover_item_with_a_new_proactive_next_step() {
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
                child_importance_scope: Some(SurrealModeScope::AllModes),
                parent_new_item: NewItemBuilder::default()
                    .summary("Parent Item")
                    .item_type(SurrealItemType::Project)
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
                .smaller_items_in_importance_order
                .len()
        );
        assert_eq!(
            &SurrealImportance {
                child_item: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Item that needs a parent")
                    .unwrap()
                    .id
                    .as_ref()
                    .unwrap()
                    .clone(),
                scope: SurrealModeScope::AllModes,
            },
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_importance_order
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
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            .item_type(SurrealItemType::Project)
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
                higher_importance_than_this: Some((SurrealModeScope::AllModes, None)),
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
                .smaller_items_in_importance_order
                .len()
        );
        assert_eq!(
            &SurrealImportance {
                child_item: surreal_tables
                    .surreal_items
                    .iter()
                    .find(|x| x.summary == "Item that needs a parent")
                    .unwrap()
                    .id
                    .as_ref()
                    .unwrap()
                    .clone(),
                scope: SurrealModeScope::AllModes,
            },
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_importance_order
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
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            .item_type(SurrealItemType::Project)
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
                higher_importance_than_this: Some((SurrealModeScope::AllModes, None)),
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
                higher_importance_than_this: Some((SurrealModeScope::AllModes, None)),
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
                higher_importance_than_this: Some((
                    SurrealModeScope::AllModes,
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .clone(),
                )),
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
                higher_importance_than_this: Some((
                    SurrealModeScope::AllModes,
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .clone(),
                )),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(
            vec![
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item at the top of the list")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 3rd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item bottom position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
            ],
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_importance_order
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn change_order_of_children() {
        // SETUP
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

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
            .item_type(SurrealItemType::Project)
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
                higher_importance_than_this: Some((SurrealModeScope::AllModes, None)),
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
                higher_importance_than_this: Some((SurrealModeScope::AllModes, None)),
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
                higher_importance_than_this: Some((
                    SurrealModeScope::AllModes,
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| {
                            x.summary
                                == "Child Item bottom position, then moved to above 2nd position"
                        })
                        .unwrap()
                        .id
                        .clone(),
                )),
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
                higher_importance_than_this: Some((
                    SurrealModeScope::AllModes,
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| {
                            x.summary
                                == "Child Item bottom position, then moved to above 2nd position"
                        })
                        .unwrap()
                        .id
                        .clone(),
                )),
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
                higher_importance_than_this: Some((
                    SurrealModeScope::AllModes,
                    surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .clone(),
                )),
            })
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();

        assert_eq!(
            vec![
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item at the top of the list")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary
                            == "Child Item bottom position, then moved to above 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 2nd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
                SurrealImportance {
                    child_item: surreal_tables
                        .surreal_items
                        .iter()
                        .find(|x| x.summary == "Child Item 3rd position")
                        .unwrap()
                        .id
                        .as_ref()
                        .unwrap()
                        .clone(),
                    scope: SurrealModeScope::AllModes,
                },
            ],
            surreal_tables
                .surreal_items
                .iter()
                .find(|x| x.summary == "Parent Item")
                .unwrap()
                .smaller_items_in_importance_order
        );

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }
}
