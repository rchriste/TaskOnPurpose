use std::fmt::Display;

use itertools::Itertools;

use crate::{display::display_mode::DisplayMode, node::mode_node::ModeNode};

use super::display_item_node::DisplayFormat;

pub(crate) struct DisplayModeNode<'s> {
    mode_node: &'s ModeNode<'s>,
    display_format: DisplayFormat,
}

impl Display for DisplayModeNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parents = self.mode_node.create_self_parent_chain();
        match self.display_format {
            DisplayFormat::MultiLineTree => {
                write!(f, "{}", DisplayMode::new(self.mode_node.get_mode()))?;
                for (j, mode) in parents.iter().skip(1).enumerate() {
                    let depth = &j;
                    writeln!(f)?;
                    for i in 0..*depth {
                        if i == *depth - 1 {
                            write!(f, "  ┗{}", DisplayMode::new(mode))?;
                        } else if parents
                            .iter()
                            .enumerate()
                            .skip(j + 1)
                            .take_while(|(d, _)| (*d - 1) >= i)
                            .any(|(d, _)| d - 1 == i)
                        {
                            write!(f, "  ┃")?;
                        } else {
                            write!(f, "   ")?;
                        }
                    }
                }
            }
            DisplayFormat::SingleLine => {
                let single_line = parents.into_iter().map(DisplayMode::new).join(" ➡ ");
                write!(f, "{}", single_line)?;
            }
        }

        Ok(())
    }
}

impl<'s> DisplayModeNode<'s> {
    pub(crate) fn new(mode_node: &'s ModeNode, display_format: DisplayFormat) -> Self {
        Self {
            mode_node,
            display_format,
        }
    }
}
