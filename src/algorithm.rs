use crate::{Algorithm, Sprite};

impl Algorithm {
    pub fn vertical(sprite: &mut Sprite) {
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = 0;
            image.y = y;
            y += image.absolute_height;
        }
    }
    pub fn horizontal(sprite: &mut Sprite) {
        let mut x = 0u32;
        for image in sprite.images.iter_mut() {
            image.y = 0;
            image.x = x;
            x += image.absolute_width;
        }
    }
    pub fn vertical_right(sprite: &mut Sprite) {
        let max_width = sprite
            .images
            .iter()
            .max_by_key(|img| img.image.height())
            .unwrap()
            .image
            .width();
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = max_width - image.image.width();
            image.y = y;
            y += image.absolute_height;
        }
    }
    pub fn horizontal_bottom(sprite: &mut Sprite) {
        let max_height = sprite
            .images
            .iter()
            .max_by_key(|img| img.image.height())
            .unwrap()
            .image
            .height();
        let mut x = 0u32;
        for image in sprite.images.iter_mut() {
            image.y = max_height - image.image.height();
            image.x = x;
            x += image.absolute_width;
        }
    }
    pub fn diagonal(sprite: &mut Sprite) {
        let mut x = 0u32;
        let mut y = 0u32;
        for image in sprite.images.iter_mut() {
            image.x = x;
            image.y = y;
            x += image.absolute_width;
            y += image.absolute_height;
        }
    }
    pub fn square(sprite: &mut Sprite) {
        // TODO: https://overflow.lunar.icu/questions/34747464/implement-graph-like-data-structure-in-rust
        // use https://smallcultfollowing.com/babysteps/blog/2015/04/06/modeling-graphs-in-rust-using-vector-indices/ instead to build the graph
        // or https://crates.io/crates/petgraph
        let mut root = SquareNode {
            width: sprite.images[0].absolute_width,
            height: sprite.images[0].absolute_height,
            ..Default::default()
        };

        // # Loot all over the images creating a binary tree
        for image in sprite.images.iter_mut() {
            let mut node = root.find(&root.clone(), image.absolute_width, image.absolute_height);
            // if let Some(mut n) = node {
            //     // use this node
            //     root.split(&mut n, image.absolute_width, image.absolute_height);
            // } else {
            //     // Grow the canvas
            //     node = root.grow(image.absolute_width, image.absolute_height);
            // }
            if let Some(n) = node {
                image.x = n.x;
                image.y = n.y;
            }
        }
    }
}

// #[derive(Debug, Clone, Default)]
// struct SquareGraph {
//     nodes: Vec<SquareNode>,
//     edges: Vec<SquareEdge>,
// }
// impl SquareGraph {
//     pub fn add_node(&mut self, node: SquareNode) -> usize {
//         let index = self.nodes.len();
//         self.nodes.push(node);
//         index
//     }
// }

#[derive(Debug, Clone, Default)]
struct SquareNode {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    used: bool,
    right: Box<SquareNode>,
    down: Box<SquareNode>,
    // first_outgoing_edge: Option<usize>,
}

impl SquareNode {
    /// Find a node to allocate this image size (width, height)
    pub fn find(&self, node: &SquareNode, width: u32, height: u32) -> Option<SquareNode> {
        if node.used {
            let mut n = node.right.find(node, width, height);
            if n.is_none() {
                n = self.down.find(node, width, height);
            }
            return n;
        } else if self.width >= width || self.height >= height {
            return Some(self.clone());
        }
        None
    }
    pub fn grow(&mut self, width: u32, height: u32) -> Option<SquareNode> {
        if self.should_grow_right(width, height) {
            self.grow_right(width, height)
        } else if self.should_grow_down(width, height) {
            self.grow_down(width, height)
        } else if self.can_grow_right(width) {
            self.grow_right(width, height)
        } else {
            // if self.can_grow_down(height) {
            self.grow_down(width, height)
        }
    }
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
    fn should_grow_right(&self, width: u32, height: u32) -> bool {
        self.can_grow_right(width) && self.height >= (self.width + width)
    }
    #[inline]
    fn should_grow_down(&self, width: u32, height: u32) -> bool {
        self.can_grow_down(height) && self.width >= (self.height + height)
    }
    /// Grow the canvas to the right.
    ///
    /// - `width`: Pixels to grow down (width)
    /// - `height`: Pixels to grow down (height).
    fn grow_right(&mut self, width: u32, height: u32) -> Option<SquareNode> {
        let old_self = self.clone();
        self.used = true;
        self.x = 0;
        self.y = 0;
        self.width += width;
        self.down = Box::new(old_self.clone());
        self.right = Box::new(SquareNode {
            x: old_self.width,
            y: 0,
            width,
            height: self.height,
            ..Default::default()
        });

        let node = self.find(self, width, height);
        if let Some(mut n) = node {
            self.split(&mut n, width, height);
            return Some(n);
        }
        None
    }

    /// Grow the canvas down.
    ///
    /// - `width`: Pixels to grow down (width)
    /// - `height`: Pixels to grow down (height).
    fn grow_down(&mut self, width: u32, height: u32) -> Option<SquareNode> {
        let old_self = self.clone();
        self.used = true;
        self.x = 0;
        self.y = 0;
        self.height += height;
        self.right = Box::new(old_self.clone());
        self.down = Box::new(SquareNode {
            x: 0,
            y: old_self.width,
            width: self.width,
            height,
            ..Default::default()
        });

        let node = self.find(self, width, height);
        if let Some(mut n) = node {
            self.split(&mut n, width, height);
            return Some(n);
        }
        None
    }

    /// Split the node to allocate a new one of this size.
    ///
    /// `node`: Node to be splitted.
    /// `width`: New node width.
    /// `height`: New node height.
    fn split(&self, node: &mut SquareNode, width: u32, height: u32) {
        node.used = true;
        node.down = Box::new(SquareNode {
            x: node.x,
            y: node.y + height,
            width: node.width,
            height: node.height - height,
            ..Default::default()
        });
        node.right = Box::new(SquareNode {
            x: node.x + width,
            y: node.y,
            width: node.width - width,
            height,
            ..Default::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grow() {
        let node = SquareNode {
            width: 5,
            height: 5,
            ..Default::default()
        };
        assert!(!node.can_grow_right(2));
        assert!(!node.can_grow_down(2));
    }
}
