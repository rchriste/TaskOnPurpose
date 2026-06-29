use ahash::HashMap;
use surrealdb::RecordId;

use crate::{
    display::{DisplayStyle, display_item::DisplayItem, display_urgency_plan::DisplayUrgency},
    node::{Filter, item_node::DependencyWithItem, item_status::ItemStatus},
};

pub(crate) fn print_completed_children(menu_for: &ItemStatus<'_>) {
    let mut completed_children = menu_for
        .get_children(Filter::Finished)
        .map(|x| x.get_item())
        .collect::<Vec<_>>();
    completed_children.sort_by(|a, b| a.get_finished_at().cmp(b.get_finished_at()));
    if !completed_children.is_empty() {
        println!("Completed Actions:");
        for child in completed_children.iter().take(8) {
            println!("  ✅{}", DisplayItem::new(child));
        }
        if completed_children.len() > 8 {
            println!("  {} more ✅", completed_children.len() - 8);
        }
    }
}

pub(crate) fn print_in_progress_children(
    menu_for: &ItemStatus<'_>,
    all_item_status: &HashMap<&RecordId, ItemStatus<'_>>,
) {
    let in_progress_children = menu_for.get_children(Filter::Active).collect::<Vec<_>>();
    if !in_progress_children.is_empty() {
        let most_important = menu_for.recursive_get_most_important_and_ready(all_item_status);
        let most_important = if let Some(most_important) = most_important {
            most_important.get_self_and_parents_flattened(Filter::Active)
        } else {
            Default::default()
        };
        println!("Smaller Actions:");
        for child in in_progress_children {
            print!("  ");
            if most_important.iter().any(|most_important| {
                most_important.get_surreal_record_id() == child.get_item().get_surreal_record_id()
            }) {
                print!("🔝");
            }
            let has_dependencies = child.get_dependencies(Filter::Active).any(|x| match x {
                // A child item being a dependency doesn't make sense to the user in this context
                DependencyWithItem::AfterChildItem(_) => false,
                _ => true,
            });
            if has_dependencies {
                print!("⏳");
            }
            let urgency_now = child
                .get_urgency_now()
                .map(|x| DisplayUrgency::new(x, DisplayStyle::Abbreviated));
            if let Some(urgency_now) = urgency_now {
                print!("{}", urgency_now);
            }
            println!("{}", DisplayItem::new(child.get_item()));
        }
    }
}
