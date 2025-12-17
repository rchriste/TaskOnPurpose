use std::fmt::Display;

use itertools::Itertools;

use crate::{display::display_mode::DisplayMode, node::mode_node::ModeNode};

use super::{
    display_item_node::DisplayFormat,
    tree_renderer::{TreeNodeWithDepth, TreeRenderer},
};

pub(crate) struct DisplayModeNode<'s> {
    mode_node: &'s ModeNode<'s>,
    display_format: DisplayFormat,
}

impl Display for DisplayModeNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parents = self.mode_node.create_parent_chain();
        match self.display_format {
            DisplayFormat::MultiLineTree | DisplayFormat::MultiLineTreeReversed => {
                // Extract depths for continuation detection
                let depths: Vec<usize> = (0..parents.len()).collect();

                // Create tree nodes from parent chain
                let tree_nodes: Vec<_> = parents
                    .iter()
                    .enumerate()
                    .map(|(idx, mode)| {
                        TreeNodeWithDepth::new(idx, DisplayMode::new(mode), idx, &depths)
                    })
                    .collect();

                let renderer = TreeRenderer::new(&tree_nodes, self.display_format);

                // Print newline before parents
                if !tree_nodes.is_empty() {
                    writeln!(f)?;
                }
                renderer.render(f)?;
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
