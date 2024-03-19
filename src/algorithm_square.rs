use crate::Sprite;

/// Square algorithm graph. It uses 2 Vecs because we cannot make the graph easily by just having recursive object.
/// Links between node are done with `edges`. It is not designed to have dynamic add/remove... so here, no problem
/// because we just build the graph on on modify some `SquareNode` properties.
///
/// See this [topic on StackOverflow](https://stackoverflow.com/questions/34747464/implement-graph-like-data-structure-in-rust)
#[derive(Debug, Clone, Default)]
struct SquareGraph {
    nodes: Vec<SquareNode>,
    // edges: Vec<SquareEdge>,
}
impl SquareGraph {
    pub fn add_node(&mut self, node: SquareNode) -> NodeIndex {
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    /// Find a node to allocate this image size (width, height).
    ///
    /// - `node_index`: node to search in
    /// - `width`: pixels to grow right (width).
    /// - `height`: pixels to grow down (height).
    pub fn find(&self, node_index: NodeIndex, width: u32, height: u32) -> Option<NodeIndex> {
        let node = &self.nodes[node_index];
        let mut result: Option<NodeIndex> = None;
        if node.used {
            result = self.find(node.right.unwrap(), width, height);
            if result.is_none() {
                result = self.find(node.down.unwrap(), width, height);
            }
        } else if node.width >= width || node.height >= height {
            return Some(node_index);
        }
        result
    }

    /// Split the node to allocate a new one of this size.
    ///
    /// `node`: Node to be splitted.
    /// `width`: New node width.
    /// `height`: New node height.
    fn split(&mut self, node_index: NodeIndex, width: u32, height: u32) {
        let node = &self.nodes[node_index];

        let down = SquareNode {
            x: node.x,
            y: node.y + height,
            width: node.width,
            height: node.height - height,
            ..Default::default()
        };
        let right = SquareNode {
            x: node.x + width,
            y: node.y,
            width: node.width - width,
            height,
            ..Default::default()
        };
        let down_index = self.add_node(down);
        let right_index = self.add_node(right);

        let node = &mut self.nodes[node_index];
        node.used = true;
        node.down = Some(down_index);
        node.right = Some(right_index);
    }

    pub fn grow(&mut self, node_index: NodeIndex, width: u32, height: u32) -> Option<NodeIndex> {
        if self.nodes[node_index].should_grow_right(width) {
            self.grow_right(node_index, width, height)
        } else if self.nodes[node_index].should_grow_down(height) {
            self.grow_down(node_index, width, height)
        } else if self.nodes[node_index].can_grow_right(width) {
            self.grow_right(node_index, width, height)
        } else {
            // if self.nodes[node_index].can_grow_down(height) {
            self.grow_down(node_index, width, height)
        }
    }
    /// Grow the canvas to the right.
    ///
    /// - `width`: Pixels to grow down (width)
    /// - `height`: Pixels to grow down (height).
    fn grow_right(&mut self, node_index: NodeIndex, width: u32, height: u32) -> Option<NodeIndex> {
        let node = &self.nodes[node_index];
        let old_self = node.clone();

        let right = self.add_node(SquareNode {
            x: old_self.width,
            y: 0,
            width,
            height: node.height,
            ..Default::default()
        });
        let down = self.add_node(old_self);

        let node = &mut self.nodes[node_index];
        node.grow(width, height);
        node.down = Some(down);
        node.right = Some(right);

        let node = self.find(node_index, width, height);
        if let Some(n) = node {
            self.split(n, width, height);
            return Some(n);
        }
        None
    }

    /// Grow the canvas down.
    ///
    /// - `width`: Pixels to grow down (width)
    /// - `height`: Pixels to grow down (height).
    fn grow_down(&mut self, node_index: NodeIndex, width: u32, height: u32) -> Option<NodeIndex> {
        let node = &self.nodes[node_index];
        let old_self = node.clone();

        let down = self.add_node(SquareNode {
            x: 0,
            y: old_self.height,
            width: node.width,
            height,
            ..Default::default()
        });
        let right = self.add_node(old_self);

        let node = &mut self.nodes[node_index];
        node.grow(width, height);
        node.down = Some(down);
        node.right = Some(right);

        let node = self.find(node_index, width, height);
        if let Some(n) = node {
            self.split(n, width, height);
            return Some(n);
        }
        None
    }
}

// https://stackoverflow.com/questions/34747464/implement-graph-like-data-structure-in-rust

type NodeIndex = usize;

/// Square node for the graph. Each node is identified by a `NodeIndex`
#[derive(Debug, Clone, Default)]
struct SquareNode {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    used: bool,
    right: Option<NodeIndex>,
    down: Option<NodeIndex>,
}

pub(crate) fn process(sprite: &mut Sprite) {
    let mut graph = SquareGraph {
        nodes: Vec::with_capacity(sprite.images.len()),
    };
    // Add the root node
    graph.add_node(SquareNode {
        width: sprite.images[0].absolute_width,
        height: sprite.images[0].absolute_height,
        ..Default::default()
    });

    // # Loot all over the images creating a binary tree
    for image in sprite.images.iter_mut() {
        let mut ni = graph.find(0, image.absolute_width, image.absolute_height);
        if let Some(n) = ni {
            // use this node
            graph.split(n, image.absolute_width, image.absolute_height);
        } else {
            // Grow the canvas
            ni = graph.grow(0, image.absolute_width, image.absolute_height);
        }
        if let Some(n) = ni {
            image.x = graph.nodes[n].x;
            image.y = graph.nodes[n].y;
        }
    }
}

impl SquareNode {
    /// If we can grow the canvas to the right
    #[inline]
    fn can_grow_right(&self, width: u32) -> bool {
        width <= self.width
    }
    /// If we can grow the canvas to the right
    #[inline]
    fn can_grow_down(&self, height: u32) -> bool {
        height <= self.height
    }
    #[inline]
    fn should_grow_right(&self, width: u32) -> bool {
        self.can_grow_right(width) && self.height >= (self.width + width)
    }
    #[inline]
    fn should_grow_down(&self, height: u32) -> bool {
        self.can_grow_down(height) && self.width >= (self.height + height)
    }

    fn grow(&mut self, width: u32, height: u32) {
        self.used = true;
        self.x = 0;
        self.y = 0;
        self.width += width;
        self.height += height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grow() {
        let mut node = SquareNode {
            width: 5,
            height: 5,
            ..Default::default()
        };
        assert!(node.can_grow_right(2));
        assert!(node.can_grow_right(5));
        assert!(!node.can_grow_right(7));
        assert!(node.can_grow_down(2));
        assert!(node.can_grow_down(5));
        assert!(!node.can_grow_down(7));

        node.height = 10;
        assert!(node.should_grow_right(2));
        assert!(node.should_grow_right(5));
        assert!(!node.should_grow_right(8));
        assert!(!node.should_grow_down(2));
        assert!(!node.should_grow_down(5));
        assert!(!node.should_grow_down(8));
        assert!(!node.should_grow_down(12));

        node.width = 20;
        assert!(!node.should_grow_right(2));
        assert!(!node.should_grow_right(5));
        assert!(!node.should_grow_right(8));
        assert!(node.should_grow_down(2));
        assert!(node.should_grow_down(5));
        assert!(node.should_grow_down(8));
        assert!(node.should_grow_down(10));
        assert!(!node.should_grow_down(10));
    }
}
