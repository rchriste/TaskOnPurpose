use std::fmt;

use super::display_item_node::DisplayFormat;

/// Renders hierarchical tree structure with line drawing characters
///
/// This renderer handles the tree structure (pipes, connectors, indentation)
/// and delegates content rendering to the provided nodes via Display trait.
pub(crate) struct TreeRenderer<'a, T: fmt::Display> {
    nodes: &'a [T],
    display_format: DisplayFormat,
}

impl<'a, T: fmt::Display> TreeRenderer<'a, T> {
    pub(crate) fn new(nodes: &'a [T], display_format: DisplayFormat) -> Self {
        Self {
            nodes,
            display_format,
        }
    }

    /// Render the tree to the formatter
    pub(crate) fn render(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.display_format {
            DisplayFormat::MultiLineTree => self.render_multiline_tree(f),
            DisplayFormat::MultiLineTreeReversed => self.render_multiline_tree_reversed(f),
            DisplayFormat::SingleLine => self.render_single_line(f),
        }
    }

    fn render_multiline_tree(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (j, node) in self.nodes.iter().enumerate() {
            if j > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", node)?;
        }
        Ok(())
    }

    fn render_multiline_tree_reversed(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (j, node) in self.nodes.iter().enumerate() {
            if j > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", node)?;
        }
        Ok(())
    }

    fn render_single_line(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (j, node) in self.nodes.iter().enumerate() {
            if j > 0 {
                write!(f, " ⬅ ")?;
            }
            write!(f, "{}", node)?;
        }
        Ok(())
    }
}

/// Helper for rendering tree structure with depth-based indentation
pub(crate) struct TreeNodeWithDepth<'a, D: fmt::Display> {
    depth: usize,
    content: D,
    /// Index of this node in the tree
    index: usize,
    /// All depths in the tree (used for continuation detection)
    all_depths: &'a [usize],
}

impl<'a, D: fmt::Display> TreeNodeWithDepth<'a, D> {
    pub(crate) fn new(depth: usize, content: D, index: usize, all_depths: &'a [usize]) -> Self {
        Self {
            depth,
            content,
            index,
            all_depths,
        }
    }

    /// Check if there's a continuation line needed at depth level `i`
    fn has_continuation_at_depth(&self, depth_level: usize) -> bool {
        self.all_depths
            .iter()
            .skip(self.index + 1)
            .any(|d| *d > depth_level)
    }
}

impl<'a, D: fmt::Display> fmt::Display for TreeNodeWithDepth<'a, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render tree structure based on depth
        for i in 0..self.depth {
            if i == self.depth - 1 {
                write!(f, "  ┗{}", self.content)?;
            } else if self.has_continuation_at_depth(i) {
                write!(f, "  ┃")?;
            } else {
                write!(f, "   ")?;
            }
        }

        // If depth is 0, still need to print the content
        if self.depth == 0 {
            write!(f, "{}", self.content)?;
        }

        Ok(())
    }
}

/// Helper for rendering reversed tree (root first, leaves last)
pub(crate) struct ReversedTreeNode<D: fmt::Display> {
    /// Position in reversed order (0 = root)
    position: usize,
    content: D,
    /// Total number of nodes
    total_nodes: usize,
}

impl<D: fmt::Display> ReversedTreeNode<D> {
    pub(crate) fn new(position: usize, content: D, total_nodes: usize) -> Self {
        Self {
            position,
            content,
            total_nodes,
        }
    }

    /// Check if there's a continuation at this indentation level
    fn has_continuation_at_position(&self, position_level: usize) -> bool {
        // In reversed tree, continuation exists if there are more nodes after us
        self.position < self.total_nodes - 1 && position_level < self.position
    }
}

impl<D: fmt::Display> fmt::Display for ReversedTreeNode<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Root item (position==0) gets no tree characters
        if self.position == 0 {
            write!(f, "{}", self.content)?;
        } else {
            // Non-root: add tree characters based on position
            for i in 0..self.position {
                if i == self.position - 1 {
                    write!(f, "  ┗{}", self.content)?;
                } else if self.has_continuation_at_position(i) {
                    write!(f, "  ┃")?;
                } else {
                    write!(f, "   ")?;
                }
            }
        }
        Ok(())
    }
}
