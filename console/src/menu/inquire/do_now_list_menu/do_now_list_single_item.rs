pub(crate) mod give_this_item_a_parent;
pub(crate) mod log_worked_on_this;
mod something_else_should_be_done_first;
pub(crate) mod state_a_smaller_action;
pub(crate) mod urgency_plan;

use std::fmt::Display;

use ahash::{HashMap, HashSet};
use better_term::Style;
use chrono::{DateTime, Utc};
use inquire::{InquireError, Select, Text};
use surrealdb::RecordId;
use tokio::sync::mpsc::Sender;
use urgency_plan::present_set_ready_and_urgency_plan_menu;

use crate::{
    base_data::{BaseData, item::Item},
    calculated_data::{CalculatedData, parent_lookup::ParentLookup},
    data_storage::surrealdb_layer::{
        data_layer_commands::DataLayerCommands,
        surreal_item::{
            Responsibility, SurrealHowMuchIsInMyControl, SurrealItemType, SurrealMotivationKind,
        },
        surreal_tables::SurrealTables,
    },
    display::{
        DisplayStyle, display_item::DisplayItem, display_item_node::DisplayItemNode,
        display_item_type::DisplayItemType, display_urgency_plan::DisplayUrgency,
    },
    menu::inquire::{
        back_menu::capture,
        default_select_page_size,
        do_now_list_menu::{
            do_now_list_single_item::{
                give_this_item_a_parent::give_this_item_a_parent,
                something_else_should_be_done_first::something_else_should_be_done_first,
                state_a_smaller_action::state_a_smaller_action,
            },
            review_item,
        },
        item_children_summary,
        select_higher_importance_than_this::select_higher_importance_than_this,
        time_spent_summary,
        update_item_summary::update_item_summary,
    },
    new_item,
    node::{
        Filter, item_node::ItemNode, item_status::ItemStatus,
        why_in_scope_and_action_with_item_status::WhyInScope,
    },
    systems::do_now_list::DoNowList,
};

use super::DisplayFormat;

enum DoNowListSingleItemSelection<'e> {
    ChangeItemType { current: &'e SurrealItemType },
    CaptureNewItem,
    GiveThisItemAParent,
    ChangeReadyAndUrgencyPlan,
    UnableToDoThisRightNow,
    SomethingElseShouldBeDoneFirst,
    ReviewItem,
    StateASmallerAction,
    WorkedOnThis,
    Finished,
    ReturnToDoNowList,
    UpdateSummary,
    SwitchToParentItem(DisplayItem<'e>, &'e ItemStatus<'e>),
    ParentToItem,
    RemoveParent(DisplayItem<'e>, &'e ItemStatus<'e>),
    SwitchToChildItem(DisplayItem<'e>, &'e ItemStatus<'e>),
    RemoveChild(DisplayItem<'e>, &'e ItemStatus<'e>),
    DebugPrintItem,
}

impl Display for DoNowListSingleItemSelection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CaptureNewItem => write!(f, "Capture New Item"),
            Self::UpdateSummary => write!(f, "Update Summary"),
            Self::SwitchToParentItem(parent_item, _) => {
                write!(f, "⇄ Select larger Reason: {}", parent_item)
            }
            Self::StateASmallerAction => {
                write!(
                    f,
                    "State a smaller Action (i.e. time spent on smaller item counts towards this item)"
                )
            }
            Self::ReviewItem => write!(f, "Review Item"),
            Self::ParentToItem => {
                write!(f, "⭱ State another larger Reason")
            }
            Self::SwitchToChildItem(child_item, _) => {
                write!(f, "⇄ Select smaller Action: {}", child_item)
            }
            Self::RemoveChild(child_item, _) => write!(f, "🚫 Remove action: {}", child_item),
            Self::RemoveParent(parent_item, _) => write!(f, "🚫 Remove reason: {}", parent_item),
            Self::DebugPrintItem => write!(f, "Debug Print Item"),
            Self::SomethingElseShouldBeDoneFirst => {
                write!(
                    f,
                    "Something else should be done first (i.e. time spent on something else does not count towards this item)"
                )
            }
            Self::ChangeItemType { current } => {
                let current_item_type = DisplayItemType::new(DisplayStyle::Full, current);
                write!(f, "Change Item Type (Currently: {})", current_item_type)
            }
            Self::GiveThisItemAParent => write!(f, "Pick a larger reason"),
            Self::UnableToDoThisRightNow => write!(f, "I am unable to do this right now"),
            Self::WorkedOnThis => write!(f, "I worked on this"),
            Self::Finished => write!(f, "I finished"),
            Self::ReturnToDoNowList => write!(f, "Return to the Do Now Menu"),
            Self::ChangeReadyAndUrgencyPlan => write!(f, "Change Ready & Urgency Plan"),
        }
    }
}

impl<'e> DoNowListSingleItemSelection<'e> {
    fn create_list(
        item_node: &'e ItemNode<'e>,
        all_items_status: &'e HashMap<&'e RecordId, ItemStatus<'e>>,
    ) -> Vec<Self> {
        let mut list = Vec::default();

        let has_no_parent = !item_node.has_parents(Filter::Active);

        if has_no_parent {
            list.push(Self::GiveThisItemAParent);
        }

        list.push(Self::CaptureNewItem);
        list.push(Self::WorkedOnThis);

        list.push(Self::Finished);

        list.push(Self::UnableToDoThisRightNow);

        list.push(Self::StateASmallerAction);

        list.push(Self::SomethingElseShouldBeDoneFirst);

        list.push(Self::ReviewItem);

        let parent_items = item_node
            .get_parents(Filter::Active)
            .map(|x| x.get_item())
            .collect::<Vec<_>>();
        if !has_no_parent {
            list.push(Self::ParentToItem);
        }
        list.extend(parent_items.iter().map(|x: &&'e Item<'e>| {
            let item_status = all_items_status
                .get(x.get_surreal_record_id())
                .expect("All items are here");
            Self::SwitchToParentItem(DisplayItem::new(x), item_status)
        }));
        list.extend(parent_items.iter().map(|x: &&'e Item<'e>| {
            let item_status = all_items_status
                .get(x.get_surreal_record_id())
                .expect("All items are here");
            Self::RemoveParent(DisplayItem::new(x), item_status)
        }));

        let child_items = item_node
            .get_children(Filter::Active)
            .map(|x| x.get_item())
            .collect::<Vec<_>>();
        list.extend(child_items.iter().map(|child: &&'e Item<'e>| {
            let child_item_status = all_items_status
                .get(child.get_surreal_record_id())
                .expect("All items are here");
            Self::SwitchToChildItem(DisplayItem::new(child), child_item_status)
        }));

        list.extend(child_items.iter().map(|child: &&'e Item<'e>| {
            let child_item_status = all_items_status
                .get(child.get_surreal_record_id())
                .expect("All items are here");
            Self::RemoveChild(DisplayItem::new(child), child_item_status)
        }));

        list.push(Self::ChangeItemType {
            current: item_node.get_type(),
        });
        list.push(Self::ChangeReadyAndUrgencyPlan);

        list.extend(vec![
            Self::UpdateSummary,
            Self::DebugPrintItem,
            Self::ReturnToDoNowList,
        ]);

        list
    }
}

pub(crate) async fn present_do_now_list_item_selected(
    menu_for: &ItemStatus<'_>,
    why_in_scope: &HashSet<WhyInScope>,
    when_selected: DateTime<Utc>, //Owns the value because you are meant to give the current time
    do_now_list: &DoNowList,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    println!();
    time_spent_summary::print_time_spent(menu_for, do_now_list);
    println!("Selected Item:");
    if let Some(urgency) = menu_for.get_urgency_now() {
        let display_urgency = DisplayUrgency::new(urgency, DisplayStyle::Abbreviated);
        println!("Urgency: {}", display_urgency);
    }
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

    let all_items_lap_highest_count = do_now_list.get_all_items_status();
    let list = DoNowListSingleItemSelection::create_list(
        menu_for.get_item_node(),
        all_items_lap_highest_count,
    );

    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();

    match selection {
        Ok(DoNowListSingleItemSelection::ChangeItemType { .. }) => {
            declare_item_type(menu_for.get_item(), send_to_data_storage_layer).await
        }
        Ok(DoNowListSingleItemSelection::CaptureNewItem) => {
            capture(send_to_data_storage_layer).await?;
            Box::pin(present_do_now_list_item_selected(
                menu_for,
                why_in_scope,
                when_selected,
                do_now_list,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(DoNowListSingleItemSelection::StateASmallerAction) => {
            state_a_smaller_action(menu_for.get_item_node(), send_to_data_storage_layer).await?;
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            let now = Utc::now();
            let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
            let calculated_data = CalculatedData::new_from_base_data(base_data);
            let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

            let menu_for = do_now_list
                .get_all_items_status()
                .get(menu_for.get_surreal_record_id())
                .expect("We will find this existing item once");

            Box::pin(present_do_now_list_item_selected(
                menu_for,
                why_in_scope,
                when_selected,
                &do_now_list,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(DoNowListSingleItemSelection::GiveThisItemAParent) => {
            give_this_item_a_parent(menu_for.get_item(), false, send_to_data_storage_layer).await
        }
        Ok(DoNowListSingleItemSelection::RemoveParent(_, selected)) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentItemRemoveParent {
                    child: menu_for.get_item().get_surreal_record_id().clone(),
                    parent_to_remove: selected.get_item().get_surreal_record_id().clone(),
                })
                .await
                .unwrap();
            Ok(())
        }
        Ok(DoNowListSingleItemSelection::RemoveChild(_, selected)) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentItemRemoveParent {
                    child: selected.get_item().get_surreal_record_id().clone(),
                    parent_to_remove: menu_for.get_item().get_surreal_record_id().clone(),
                })
                .await
                .unwrap();
            Ok(())
        }
        Ok(DoNowListSingleItemSelection::UnableToDoThisRightNow) => {
            let base_data = do_now_list.get_base_data();
            present_set_ready_and_urgency_plan_menu(menu_for, base_data, send_to_data_storage_layer)
                .await
        }
        Ok(DoNowListSingleItemSelection::SomethingElseShouldBeDoneFirst) => {
            something_else_should_be_done_first(menu_for.get_item(), send_to_data_storage_layer)
                .await
        }
        Ok(DoNowListSingleItemSelection::ReviewItem) => {
            review_item::present_review_item_menu(menu_for, send_to_data_storage_layer).await
        }
        Ok(DoNowListSingleItemSelection::WorkedOnThis) => {
            let base_data = do_now_list.get_base_data();
            present_set_ready_and_urgency_plan_menu(
                menu_for,
                base_data,
                send_to_data_storage_layer,
            )
            .await?;
            log_worked_on_this::log_worked_on_this(
                menu_for,
                why_in_scope,
                &when_selected,
                send_to_data_storage_layer,
            )
            .await
        }
        Ok(DoNowListSingleItemSelection::Finished) => {
            finish_do_now_item(
                menu_for,
                why_in_scope,
                do_now_list,
                Utc::now(),
                send_to_data_storage_layer,
            )
            .await?;
            log_worked_on_this::log_worked_on_this(
                menu_for,
                why_in_scope,
                &when_selected,
                send_to_data_storage_layer,
            )
            .await
        }
        Ok(DoNowListSingleItemSelection::ChangeReadyAndUrgencyPlan) => {
            let base_data = do_now_list.get_base_data();
            present_set_ready_and_urgency_plan_menu(menu_for, base_data, send_to_data_storage_layer)
                .await
        }
        Ok(DoNowListSingleItemSelection::UpdateSummary) => {
            update_item_summary(menu_for.get_item(), send_to_data_storage_layer).await?;
            //After updating the summary we want to stay on the same item with the same times
            Box::pin(present_do_now_list_item_selected(
                menu_for,
                why_in_scope,
                when_selected,
                do_now_list,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(DoNowListSingleItemSelection::SwitchToParentItem(_, selected))
        | Ok(DoNowListSingleItemSelection::SwitchToChildItem(_, selected)) => {
            Box::pin(present_do_now_list_item_selected(
                selected,
                why_in_scope,
                chrono::Utc::now(),
                do_now_list,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(DoNowListSingleItemSelection::ParentToItem) => {
            parent_to_item(menu_for.get_item(), send_to_data_storage_layer).await
        }
        Ok(DoNowListSingleItemSelection::DebugPrintItem) => {
            println!("{:?}", menu_for);
            Ok(())
        }
        Ok(DoNowListSingleItemSelection::ReturnToDoNowList)
        | Err(InquireError::OperationCanceled) => Ok(()), //Nothing to do we just want to return to the bullet list
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

enum FinishSelection<'e> {
    CreateNextStepWithParent(&'e Item<'e>),
    GoToParent(&'e Item<'e>),
    CaptureNewItem,
    ReturnToDoNowList,
}

impl Display for FinishSelection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FinishSelection::CreateNextStepWithParent(parent) => write!(
                f,
                "Create Next Step with Parent: {}",
                DisplayItem::new(parent)
            ),
            FinishSelection::GoToParent(parent) => {
                write!(f, "Go to Parent: {}", DisplayItem::new(parent))
            }
            FinishSelection::CaptureNewItem => write!(f, "Capture New Item"),
            FinishSelection::ReturnToDoNowList => write!(f, "🔙 Return to Do Now List"),
        }
    }
}

impl<'e> FinishSelection<'e> {
    fn make_list(parents: &[&'e Item<'e>]) -> Vec<Self> {
        let mut list = Vec::default();
        list.push(Self::ReturnToDoNowList);
        list.push(Self::CaptureNewItem);
        list.extend(
            parents
                .iter()
                .flat_map(|x| vec![Self::CreateNextStepWithParent(x), Self::GoToParent(x)]),
        );
        list
    }
}

async fn finish_do_now_item(
    finish_this: &ItemStatus<'_>,
    why_in_scope: &HashSet<WhyInScope>,
    do_now_list: &DoNowList,
    now: DateTime<Utc>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let when_this_function_was_called = now;
    send_to_data_storage_layer
        .send(DataLayerCommands::FinishItem {
            item: finish_this.get_surreal_record_id().clone(),
            when_finished: now.into(),
        })
        .await
        .unwrap();

    let list = FinishSelection::make_list(
        &finish_this
            .get_parents(Filter::Active)
            .map(|x| x.get_item())
            .collect::<Vec<_>>(),
    );
    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();

    match selection {
        Ok(FinishSelection::CaptureNewItem) => capture(send_to_data_storage_layer).await,
        Ok(FinishSelection::CreateNextStepWithParent(parent)) => {
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            let now = Utc::now();
            let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
            let items = base_data.get_items();
            let parents = ParentLookup::new(items);
            let events = base_data.get_events();
            let parent_surreal_record_id = parent.get_surreal_record_id();
            let time_spent_log = base_data.get_time_spent_log();
            let updated_parent = ItemNode::new(
                items
                    .get(parent_surreal_record_id)
                    .expect("Should be there"),
                items,
                &parents,
                events,
                time_spent_log,
            );

            state_a_smaller_action(&updated_parent, send_to_data_storage_layer).await?;

            //Recursively call as a way of creating a loop, we don't want to return to the main do now list
            Box::pin(finish_do_now_item(
                finish_this,
                why_in_scope,
                do_now_list,
                Utc::now(),
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(FinishSelection::GoToParent(parent)) => {
            let surreal_tables = SurrealTables::new(send_to_data_storage_layer)
                .await
                .unwrap();
            let now = Utc::now();
            let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
            let calculated_data = CalculatedData::new_from_base_data(base_data);
            let parent_surreal_record_id = parent.get_surreal_record_id();
            let updated_parent = calculated_data
                .get_items_status()
                .get(parent_surreal_record_id)
                .expect("We will find this existing item once");

            Box::pin(present_do_now_list_item_selected(
                updated_parent,
                why_in_scope,
                when_this_function_was_called,
                do_now_list,
                send_to_data_storage_layer,
            ))
            .await
        }
        Ok(FinishSelection::ReturnToDoNowList) => Ok(()),
        Err(InquireError::OperationCanceled) => {
            todo!("This should undo the finish and put the item back to what it was before")
        }
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

async fn parent_to_item(
    parent_this: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let raw_data = SurrealTables::new(send_to_data_storage_layer)
        .await
        .unwrap();
    let now = Utc::now();
    let base_data = BaseData::new_from_surreal_tables(raw_data, now);
    let items = base_data.get_items();
    let parent_lookup = ParentLookup::new(items);
    let active_items = base_data.get_active_items();
    let events = base_data.get_events();
    let time_spent_log = base_data.get_time_spent_log();
    let item_nodes = active_items
        .iter()
        .map(|x| ItemNode::new(x, items, &parent_lookup, events, time_spent_log))
        .collect::<Vec<_>>();
    let list = DisplayItemNode::make_list(&item_nodes, Filter::Active, DisplayFormat::SingleLine);

    let selection = Select::new("Type to Search or Press Esc to enter a new one", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(display_item) => {
            let item_node: &ItemNode = display_item.get_item_node();
            let higher_importance_than_this = if item_node.has_children(Filter::Active) {
                let items = item_node
                    .get_children(Filter::Active)
                    .map(|x| x.get_item())
                    .collect::<Vec<_>>();
                select_higher_importance_than_this(&items, None)
            } else {
                None
            };
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentItemWithExistingItem {
                    child: parent_this.get_surreal_record_id().clone(),
                    parent: item_node.get_surreal_record_id().clone(),
                    higher_importance_than_this,
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::InvalidConfiguration(_)) => {
            parent_to_new_item(parent_this, send_to_data_storage_layer).await
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) enum ItemTypeSelection {
    Action,
    Goal,
    Idea,
    MotivationCore,
    MotivationNonCore,
    MotivationNeither,
    NormalHelp,
}

impl Display for ItemTypeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Action => write!(f, "Task or Step 🪜"),
            Self::Goal => write!(f, "Commitment or Project 🪧"),
            Self::Idea => write!(f, "Idea or Thought 💡"),
            Self::MotivationCore => {
                write!(f, "Core Motivational Purpose 🎯🏢")
            }
            Self::MotivationNonCore => {
                write!(f, "Non-Core Motivational Purpose 🎯🧹")
            }
            Self::MotivationNeither => {
                write!(f, "Neither Core nor Non-Core Motivational Purpose 🎯🚫")
            }
            Self::NormalHelp => write!(f, "❓ Help"),
        }
    }
}

impl ItemTypeSelection {
    pub(crate) fn create_list() -> Vec<Self> {
        vec![
            Self::Action,
            Self::Goal,
            Self::Idea,
            Self::MotivationCore,
            Self::MotivationNonCore,
            Self::MotivationNeither,
            Self::NormalHelp,
        ]
    }

    pub(crate) fn create_new_item_prompt_user_for_summary(&self) -> new_item::NewItem {
        let summary = Text::new("Enter Summary ⍠").prompt().unwrap();
        self.create_new_item(summary)
    }

    pub(crate) fn create_new_item(&self, summary: String) -> new_item::NewItem {
        let mut new_item_builder = new_item::NewItemBuilder::default();
        let new_item_builder = new_item_builder.summary(summary);
        let new_item_builder = match self {
            ItemTypeSelection::Action => new_item_builder
                .responsibility(Responsibility::ProactiveActionToTake)
                .item_type(SurrealItemType::Action),
            ItemTypeSelection::Goal => new_item_builder
                .responsibility(Responsibility::ProactiveActionToTake)
                .item_type(SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default())),
            ItemTypeSelection::MotivationCore => new_item_builder
                .responsibility(Responsibility::ReactiveBeAvailableToAct)
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork)),
            ItemTypeSelection::MotivationNonCore => new_item_builder
                .responsibility(Responsibility::ReactiveBeAvailableToAct)
                .item_type(SurrealItemType::Motivation(
                    SurrealMotivationKind::NonCoreWork,
                )),
            ItemTypeSelection::MotivationNeither => new_item_builder
                .responsibility(Responsibility::ReactiveBeAvailableToAct)
                .item_type(SurrealItemType::Motivation(
                    SurrealMotivationKind::DoesNotFitInCoreOrNonCore,
                )),
            ItemTypeSelection::Idea => new_item_builder
                .responsibility(Responsibility::ProactiveActionToTake)
                .item_type(SurrealItemType::IdeaOrThought),
            ItemTypeSelection::NormalHelp => {
                panic!("NormalHelp should be handled before this point")
            }
        };
        new_item_builder
            .build()
            .expect("Filled out required fields")
    }

    pub(crate) fn print_normal_help() {
        println!("{}Step{}", Style::default().bold(), Style::default());
        println!("A task to do and an action or step to take.");
        println!(
            "{}The emoji is a ladder 🪜 with steps.{}",
            Style::default().italic(),
            Style::default()
        );
        println!();
        println!(
            "{}Multi-Step Project{}",
            Style::default().bold(),
            Style::default()
        );
        println!(
            "A milestone or hopeful outcome that should be broken down to smaller steps to accomplish."
        );
        println!(
            "{}The emoji is a Milestone sign 🪧 or goal post.{}",
            Style::default().italic(),
            Style::default()
        );
        println!();
        println!(
            "{}Motivational Purpose or Reason{}",
            Style::default().bold(),
            Style::default()
        );
        println!(
            "For stating that the item captured is a long standing value or reason for doing something."
        );
        println!(
            "{}Emoji is a target 🎯 that provides something to aim for.{}",
            Style::default().italic(),
            Style::default()
        );
        println!(
            "\t🏢Core🏢 is for your work that makes you productive. In your business life this is for work that is a core reason you were hired. When you consider or report on what was accomplished you are generally mentioning things in this category."
        );
        println!(
            "\t🧹Non-Core🧹 is for work that might still be essential to do but is considered more of a background thing. Maybe not doing these items is a problem but doing them is not enough to consider that you had a productive day."
        );
        println!();
    }
}

pub(crate) async fn parent_to_new_item(
    parent_this: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = ItemTypeSelection::create_list();

    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ItemTypeSelection::NormalHelp) => {
            ItemTypeSelection::print_normal_help();
            Box::pin(parent_to_new_item(parent_this, send_to_data_storage_layer)).await
        }
        Ok(item_type_selection) => {
            let new_item = item_type_selection.create_new_item_prompt_user_for_summary();
            send_to_data_storage_layer
                .send(DataLayerCommands::ParentNewItemWithAnExistingChildItem {
                    child: parent_this.get_surreal_record_id().clone(),
                    parent_new_item: new_item,
                })
                .await
                .unwrap();
            Ok(())
        }
        Err(InquireError::OperationCanceled) => todo!(),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

pub(crate) async fn declare_item_type(
    item: &Item<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = ItemTypeSelection::create_list();

    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ItemTypeSelection::Action) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ProactiveActionToTake,
                    SurrealItemType::Action,
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::Goal) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ProactiveActionToTake,
                    SurrealItemType::Goal(SurrealHowMuchIsInMyControl::default()),
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::MotivationCore) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ReactiveBeAvailableToAct,
                    SurrealItemType::Motivation(SurrealMotivationKind::CoreWork),
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::MotivationNonCore) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ReactiveBeAvailableToAct,
                    SurrealItemType::Motivation(SurrealMotivationKind::NonCoreWork),
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::MotivationNeither) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ReactiveBeAvailableToAct,
                    SurrealItemType::Motivation(SurrealMotivationKind::DoesNotFitInCoreOrNonCore),
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::Idea) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::UpdateResponsibilityAndItemType(
                    item.get_surreal_record_id().clone(),
                    Responsibility::ProactiveActionToTake,
                    SurrealItemType::IdeaOrThought,
                ))
                .await
                .unwrap();
            Ok(())
        }
        Ok(ItemTypeSelection::NormalHelp) => {
            ItemTypeSelection::print_normal_help();
            Box::pin(declare_item_type(item, send_to_data_storage_layer)).await
        }
        Err(InquireError::OperationCanceled) => todo!(),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

enum IsAPersonOrGroupAroundSelection {
    Yes,
    No,
}

impl Display for IsAPersonOrGroupAroundSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IsAPersonOrGroupAroundSelection::Yes => write!(f, "Yes"),
            IsAPersonOrGroupAroundSelection::No => write!(f, "No"),
        }
    }
}

impl IsAPersonOrGroupAroundSelection {
    fn create_list() -> Vec<Self> {
        vec![Self::Yes, Self::No]
    }
}

pub(crate) async fn present_is_person_or_group_around_menu(
    person_or_group_node: &ItemNode<'_>,
    send_to_data_storage_layer: &Sender<DataLayerCommands>,
) -> Result<(), ()> {
    let list = IsAPersonOrGroupAroundSelection::create_list();

    let selection = Select::new("Select from the below list|", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(IsAPersonOrGroupAroundSelection::Yes) => {
            send_to_data_storage_layer
                .send(DataLayerCommands::FinishItem {
                    item: person_or_group_node.get_surreal_record_id().clone(),
                    when_finished: Utc::now().into(),
                })
                .await
                .unwrap();
            Ok(())
        }
        Ok(IsAPersonOrGroupAroundSelection::No) => todo!(),
        Err(InquireError::OperationCanceled) => todo!(),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}
