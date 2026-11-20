use crate::{
    Sprite,
    util::{absolute_length, horizontal_spacing, vertical_spacing},
};

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
        } else if node.width >= width && node.height >= height {
            return Some(node_index);
        }
        result
    }

    /// Split the node to allocate a new one of this size.
    ///
    /// `node`: Node to be split.
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
        let right_index = self.add_node(right);
        let down_index = self.add_node(down);

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
#[derive(Debug, Clone, PartialEq, Default)]
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
    if sprite.images.is_empty() {
        return;
    }
    let mut graph = SquareGraph {
        nodes: Vec::with_capacity(sprite.images.len() * 2),
    };
    // Add the root node
    graph.add_node(SquareNode {
        width: absolute_length(sprite.images[0].image.width(), 0, 1f32),
        height: absolute_length(sprite.images[0].image.height(), 0, 1f32),
        ..Default::default()
    });

    // # Loot all over the images creating a binary tree
    for image in sprite.images.iter_mut() {
        let absolute_width = absolute_length(
            image.image.width(),
            horizontal_spacing(
                sprite.padding.left,
                sprite.padding.right,
                sprite.margin.left,
                sprite.margin.right,
            ),
            sprite.ratios[0],
        );
        let absolute_height = absolute_length(
            image.image.height(),
            vertical_spacing(
                sprite.padding.top,
                sprite.padding.bottom,
                sprite.margin.top,
                sprite.margin.bottom,
            ),
            sprite.ratios[0],
        );
        let mut ni = graph.find(0, absolute_width, absolute_height);
        if let Some(n) = ni {
            // use this node
            graph.split(n, absolute_width, absolute_height);
        } else {
            // Grow the canvas
            ni = graph.grow(0, absolute_width, absolute_height);
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
        assert!(!node.should_grow_down(11));
    }

    #[test]
    fn test_graph() {
        let mut g = SquareGraph {
            nodes: Vec::with_capacity(1),
        };
        let root_index = g.add_node(SquareNode {
            width: 10,
            height: 10,
            ..Default::default()
        });
        assert_eq!(root_index, 0);
        // TODO: better tests
        g.split(0, 5, 5);
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(
            g.nodes[1],
            SquareNode {
                x: 5,
                width: 5,
                height: 5,
                ..Default::default()
            }
        );
        assert_eq!(
            g.nodes[2],
            SquareNode {
                y: 5,
                width: 10,
                height: 5,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_graph_2() {
        let mut g = SquareGraph {
            nodes: Vec::with_capacity(7),
        };
        let root_index = g.add_node(SquareNode {
            width: 10,
            height: 10,
            ..Default::default()
        });
        // TODO: better tests
        assert_eq!(g.grow(root_index, 8, 4), Some(1));
        assert_eq!(
            g.nodes[1],
            SquareNode {
                x: 10,
                y: 0,
                width: 8,
                height: 10,
                used: true,
                right: Some(3),
                down: Some(4)
            }
        );
    }

    #[test]
    fn test_node_grow() {
        let mut node = SquareNode {
            x: 5,
            y: 10,
            width: 20,
            height: 30,
            used: false,
            right: None,
            down: None,
        };

        node.grow(10, 15);

        assert!(node.used);
        assert_eq!(node.x, 0);
        assert_eq!(node.y, 0);
        assert_eq!(node.width, 30);
        assert_eq!(node.height, 45);
    }

    #[test]
    fn test_graph_find() {
        let mut g = SquareGraph::default();

        // Add root node
        g.add_node(SquareNode {
            width: 100,
            height: 100,
            ..Default::default()
        });

        // Split the root
        g.split(0, 50, 50);

        // Should find a node that fits
        let found = g.find(0, 40, 40);
        assert!(found.is_some());

        // Root node is now used, so find should search children
        assert!(g.nodes[0].used);
    }

    #[test]
    fn test_graph_grow_down() {
        let mut g = SquareGraph::default();

        // Add a wide root node (should prefer growing down)
        g.add_node(SquareNode {
            width: 100,
            height: 50,
            ..Default::default()
        });

        // This should grow down because width >= height + new_height
        let result = g.grow(0, 30, 30);
        assert!(result.is_some());
    }

    #[test]
    fn test_graph_grow_right() {
        let mut g = SquareGraph::default();

        // Add a tall root node (should prefer growing right)
        g.add_node(SquareNode {
            width: 50,
            height: 100,
            ..Default::default()
        });

        // This should grow right because height >= width + new_width
        let result = g.grow(0, 30, 30);
        assert!(result.is_some());
    }

    #[test]
    fn test_process_single_image() {
        use crate::{Config, ImageAndPosition, Sprite};
        use image::DynamicImage;

        let config = Config::default();
        let mut sprite = Sprite {
            config: config.clone(),
            path: std::path::PathBuf::from("test"),
            ratios: vec![1.0],
            images: vec![ImageAndPosition {
                path: String::from("test.png"),
                image: DynamicImage::new_rgba8(50, 30),
                x: 0,
                y: 0,
                computed_width: 50,
                computed_height: 30,
                aliases: vec![],
            }],
            margin: crate::util::Spacing::default(),
            padding: crate::util::Spacing::default(),
        };

        process(&mut sprite);

        // Single image should be at origin
        assert_eq!(sprite.images[0].x, 0);
        assert_eq!(sprite.images[0].y, 0);
    }

    #[test]
    fn test_process_empty_images() {
        use crate::{Config, Sprite};

        let config = Config::default();
        let mut sprite = Sprite {
            config: config.clone(),
            path: std::path::PathBuf::from("test"),
            ratios: vec![1.0],
            images: vec![],
            margin: crate::util::Spacing::default(),
            padding: crate::util::Spacing::default(),
        };

        // Should not panic on empty images
        process(&mut sprite);
        assert!(sprite.images.is_empty());
    }
}
