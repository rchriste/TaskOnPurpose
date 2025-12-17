pub(crate) mod display_action_with_item_status;
pub(crate) mod display_dependencies_with_item_node;
pub(crate) mod display_duration;
pub(crate) mod display_duration_one_unit;
pub(crate) mod display_item;
pub(crate) mod display_item_node;
pub(crate) mod display_item_status;
pub(crate) mod display_item_status_with_urgency;
pub(crate) mod display_item_type;
pub(crate) mod display_mode;
pub(crate) mod display_mode_node;
pub(crate) mod display_scheduled_item;
pub(crate) mod display_urgency_level_item_with_item_status;
pub(crate) mod display_urgency_plan;
pub(crate) mod display_why_in_scope_and_action_with_item_status;
pub(crate) mod tree_renderer;

pub(crate) enum DisplayStyle {
    Abbreviated,
    Full,
}
