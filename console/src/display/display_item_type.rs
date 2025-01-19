use std::fmt::Display;

use crate::data_storage::surrealdb_layer::surreal_item::SurrealItemType;

use super::DisplayStyle;

pub(crate) struct DisplayItemType<'s> {
    item_type: &'s SurrealItemType,
    style: DisplayStyle,
}

impl Display for DisplayItemType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.item_type {
            SurrealItemType::Project => {
                write!(f, "🪧")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " Commitment or Project"),
                }
            }
            SurrealItemType::Motivation => {
                write!(f, "🎯")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " Motivational Purpose"),
                }
            }
            SurrealItemType::Action => {
                write!(f, "🪜")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " Task or Step"),
                }
            }
            SurrealItemType::Undeclared => match self.style {
                DisplayStyle::Abbreviated => Ok(()),
                DisplayStyle::Full => write!(f, "❓ Undeclared"),
            },
            SurrealItemType::PersonOrGroup => {
                write!(f, "👤")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " Person or Group"),
                }
            }
            SurrealItemType::IdeaOrThought => {
                write!(f, "💡")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " Idea or Thought"),
                }
            }
        }
    }
}

impl<'s> DisplayItemType<'s> {
    pub(crate) fn new(style: DisplayStyle, item_type: &'s SurrealItemType) -> Self {
        DisplayItemType { item_type, style }
    }
}
