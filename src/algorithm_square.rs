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
    /// Per node: upper bound `(width, height)` of the free nodes in its subtree (each
    /// component maximised separately). Searches skip subtrees where no block can fit, so
    /// filled regions are not walked again for every image.
    free: Vec<(u32, u32)>,
    /// Per node: its parent, to update `free` up the tree.
    parent: Vec<Option<NodeIndex>>,
}
impl SquareGraph {
    pub fn add_node(&mut self, node: SquareNode) -> NodeIndex {
        let index = self.nodes.len();
        let bound = self.free_bound(&node);
        for child in [node.right, node.down].into_iter().flatten() {
            self.parent[child] = Some(index);
        }
        self.nodes.push(node);
        self.free.push(bound);
        self.parent.push(None);
        index
    }

    /// Free-space bound of `node`: its own size when free, its children's otherwise.
    ///
    /// Empty leaves (0 wide or 0 tall, left over by splits) count as `(0, 0)`: they can never
    /// hold a block, and their single non-zero side would otherwise inflate the bound of
    /// full regions.
    fn free_bound(&self, node: &SquareNode) -> (u32, u32) {
        if !node.used {
            return if node.width == 0 || node.height == 0 {
                (0, 0)
            } else {
                (node.width, node.height)
            };
        }
        [node.right, node.down]
            .into_iter()
            .flatten()
            .map(|child| self.free[child])
            .fold((0, 0), |(width, height), (child_w, child_h)| {
                (width.max(child_w), height.max(child_h))
            })
    }

    /// Recompute the free-space bound of `index` and its ancestors, as far as it changes.
    fn refresh(&mut self, mut index: NodeIndex) {
        loop {
            let bound = self.free_bound(&self.nodes[index]);
            if self.free[index] == bound {
                return;
            }
            self.free[index] = bound;
            match self.parent[index] {
                Some(parent) => index = parent,
                None => return,
            }
        }
    }

    /// Make `parent` (already updated) the parent of its children and refresh its bound.
    fn adopt_children(&mut self, parent: NodeIndex) {
        let node = &self.nodes[parent];
        for child in [node.right, node.down].into_iter().flatten() {
            self.parent[child] = Some(parent);
        }
        self.free[parent] = self.free_bound(&self.nodes[parent]);
    }

    /// Find a node to allocate this image size (width, height).
    ///
    /// - `node_index`: node to search in
    /// - `width`: pixels to grow right (width).
    /// - `height`: pixels to grow down (height).
    ///
    /// Depth-first, right subtree before the down one. Iterative: a degenerate layout makes
    /// the tree as deep as the number of images, which would overflow the stack of a
    /// recursive search.
    pub fn find(&self, node_index: NodeIndex, width: u32, height: u32) -> Option<NodeIndex> {
        // Images are at least 1×1, so are blocks: empty leaves never match (see `free_bound`).
        debug_assert!(width > 0 && height > 0, "empty {width}x{height} block");
        let mut pending = vec![node_index];
        while let Some(index) = pending.pop() {
            // Nothing below can fit: skipped without changing which node is found first.
            let (free_w, free_h) = self.free[index];
            if free_w < width || free_h < height {
                continue;
            }
            let node = &self.nodes[index];
            if !node.used {
                return Some(index);
            }
            // Pushed in reverse order: the right subtree is searched first.
            pending.extend(node.down);
            pending.extend(node.right);
        }
        None
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
        self.parent[right_index] = Some(node_index);
        self.parent[down_index] = Some(node_index);
        self.refresh(node_index);
    }

    /// Grow the canvas so that a `width`×`height` block fits, allocate it and return its node.
    ///
    /// The root must be at least as wide and as tall as every block (see [`process`]):
    /// growing in either direction is then always possible, so this never fails.
    pub fn grow(&mut self, root_index: NodeIndex, width: u32, height: u32) -> NodeIndex {
        let root = &self.nodes[root_index];
        debug_assert!(
            root.can_grow_right(height) && root.can_grow_down(width),
            "square root ({}x{}) is smaller than a {}x{} block",
            root.width,
            root.height,
            width,
            height
        );
        if root.should_grow_right(width, height) {
            self.grow_right(root_index, width, height)
        } else if root.should_grow_down(width, height) {
            self.grow_down(root_index, width, height)
        } else if root.can_grow_right(height) {
            self.grow_right(root_index, width, height)
        } else {
            self.grow_down(root_index, width, height)
        }
    }

    /// Grow the canvas to the right by `width` pixels and allocate the block in the new column.
    fn grow_right(&mut self, root_index: NodeIndex, width: u32, height: u32) -> NodeIndex {
        let old_root = self.nodes[root_index].clone();

        let right = self.add_node(SquareNode {
            x: old_root.width,
            y: 0,
            width,
            height: old_root.height,
            ..Default::default()
        });
        let down = self.add_node(old_root);

        let root = &mut self.nodes[root_index];
        root.used = true;
        root.width += width;
        root.down = Some(down);
        root.right = Some(right);
        self.adopt_children(root_index);

        self.split(right, width, height);
        right
    }

    /// Grow the canvas down by `height` pixels and allocate the block in the new row.
    fn grow_down(&mut self, root_index: NodeIndex, width: u32, height: u32) -> NodeIndex {
        let old_root = self.nodes[root_index].clone();

        let down = self.add_node(SquareNode {
            x: 0,
            y: old_root.height,
            width: old_root.width,
            height,
            ..Default::default()
        });
        let right = self.add_node(old_root);

        let root = &mut self.nodes[root_index];
        root.used = true;
        root.height += height;
        root.down = Some(down);
        root.right = Some(right);
        self.adopt_children(root_index);

        self.split(down, width, height);
        down
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
    let hspace = horizontal_spacing(
        sprite.padding.left,
        sprite.padding.right,
        sprite.margin.left,
        sprite.margin.right,
    );
    let vspace = vertical_spacing(
        sprite.padding.top,
        sprite.padding.bottom,
        sprite.margin.top,
        sprite.margin.bottom,
    );
    // Sizes and spacing are both in 1× (CSS pixel) units.
    let ratio = 1.0;
    let blocks: Vec<(u32, u32)> = sprite
        .images
        .iter()
        .map(|image| {
            (
                absolute_length(image.computed_width, hspace, ratio),
                absolute_length(image.computed_height, vspace, ratio),
            )
        })
        .collect();

    // The root starts as wide as the widest block and as tall as the tallest one, so every
    // block fits in either growth direction whatever the ordering (see `SquareGraph::grow`).
    let capacity = sprite.images.len() * 3 + 1;
    let mut graph = SquareGraph {
        nodes: Vec::with_capacity(capacity),
        free: Vec::with_capacity(capacity),
        parent: Vec::with_capacity(capacity),
    };
    let root = graph.add_node(SquareNode {
        width: blocks.iter().map(|block| block.0).max().unwrap_or(0),
        height: blocks.iter().map(|block| block.1).max().unwrap_or(0),
        ..Default::default()
    });

    for (image, &(width, height)) in sprite.images.iter_mut().zip(&blocks) {
        let node_index = match graph.find(root, width, height) {
            Some(free) => {
                graph.split(free, width, height);
                free
            }
            None => graph.grow(root, width, height),
        };
        image.x = graph.nodes[node_index].x;
        image.y = graph.nodes[node_index].y;
    }
}

impl SquareNode {
    /// A new column on the right spans the canvas height, so the block must not be taller.
    #[inline]
    fn can_grow_right(&self, height: u32) -> bool {
        height <= self.height
    }
    /// A new row at the bottom spans the canvas width, so the block must not be wider.
    #[inline]
    fn can_grow_down(&self, width: u32) -> bool {
        width <= self.width
    }
    /// Grow right while the canvas stays taller than wide, to keep it roughly square.
    #[inline]
    fn should_grow_right(&self, width: u32, height: u32) -> bool {
        self.can_grow_right(height) && self.height >= (self.width + width)
    }
    /// Grow down while the canvas stays wider than tall, to keep it roughly square.
    #[inline]
    fn should_grow_down(&self, width: u32, height: u32) -> bool {
        self.can_grow_down(width) && self.width >= (self.height + height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, ImageAndPosition, Sprite};
    use image::DynamicImage;

    fn sprite_of(sizes: &[(u32, u32)]) -> Sprite {
        Sprite {
            config: Config::default(),
            path: std::path::PathBuf::from("test"),
            ratios: vec![1.0],
            images: sizes
                .iter()
                .enumerate()
                .map(|(index, &(width, height))| ImageAndPosition {
                    path: format!("{index}.png"),
                    image: DynamicImage::new_rgba8(width, height),
                    computed_width: width,
                    computed_height: height,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn assert_no_overlap(sprite: &Sprite) {
        for (index, first) in sprite.images.iter().enumerate() {
            for second in &sprite.images[index + 1..] {
                let disjoint = first.x + first.computed_width <= second.x
                    || second.x + second.computed_width <= first.x
                    || first.y + first.computed_height <= second.y
                    || second.y + second.computed_height <= first.y;
                assert!(
                    disjoint,
                    "{} at ({},{}) overlaps {} at ({},{})",
                    first.path, first.x, first.y, second.path, second.x, second.y
                );
            }
        }
    }

    #[test]
    fn test_can_and_should_grow() {
        let node = SquareNode {
            width: 5,
            height: 10,
            ..Default::default()
        };
        // A new column is as tall as the canvas, a new row as wide.
        assert!(node.can_grow_right(10));
        assert!(!node.can_grow_right(11));
        assert!(node.can_grow_down(5));
        assert!(!node.can_grow_down(6));

        // Taller than wide: prefer growing right while it stays so.
        assert!(node.should_grow_right(5, 10));
        assert!(!node.should_grow_right(6, 10));
        assert!(!node.should_grow_right(5, 11));
        assert!(!node.should_grow_down(5, 1));
    }

    #[test]
    fn test_split() {
        let mut graph = SquareGraph::default();
        graph.add_node(SquareNode {
            width: 10,
            height: 10,
            ..Default::default()
        });
        graph.split(0, 5, 5);
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes[0].used);
        assert_eq!(
            graph.nodes[1],
            SquareNode {
                x: 5,
                width: 5,
                height: 5,
                ..Default::default()
            }
        );
        assert_eq!(
            graph.nodes[2],
            SquareNode {
                y: 5,
                width: 10,
                height: 5,
                ..Default::default()
            }
        );
        assert_eq!(graph.find(0, 4, 4), Some(1));
        assert_eq!(graph.find(0, 10, 5), Some(2));
        assert_eq!(graph.find(0, 10, 6), None);
    }

    #[test]
    fn test_grow_right_only_widens() {
        let mut graph = SquareGraph::default();
        graph.add_node(SquareNode {
            width: 10,
            height: 10,
            ..Default::default()
        });
        let node = graph.grow_right(0, 4, 6);
        assert_eq!((graph.nodes[0].width, graph.nodes[0].height), (14, 10));
        assert_eq!((graph.nodes[node].x, graph.nodes[node].y), (10, 0));
        assert!(graph.nodes[node].used);
    }

    #[test]
    fn test_grow_down_only_heightens() {
        let mut graph = SquareGraph::default();
        graph.add_node(SquareNode {
            width: 10,
            height: 10,
            ..Default::default()
        });
        let node = graph.grow_down(0, 6, 4);
        assert_eq!((graph.nodes[0].width, graph.nodes[0].height), (10, 14));
        assert_eq!((graph.nodes[node].x, graph.nodes[node].y), (0, 10));
        assert!(graph.nodes[node].used);
    }

    #[test]
    fn test_grow_prefers_keeping_square() {
        let mut graph = SquareGraph::default();
        graph.add_node(SquareNode {
            width: 100,
            height: 50,
            ..Default::default()
        });
        // Wide canvas: grow down.
        let node = graph.grow(0, 30, 30);
        assert_eq!((graph.nodes[node].x, graph.nodes[node].y), (0, 50));

        let mut graph = SquareGraph::default();
        graph.add_node(SquareNode {
            width: 50,
            height: 100,
            ..Default::default()
        });
        // Tall canvas: grow right.
        let node = graph.grow(0, 30, 30);
        assert_eq!((graph.nodes[node].x, graph.nodes[node].y), (50, 0));
    }

    #[test]
    fn test_process_single_image() {
        let mut sprite = sprite_of(&[(50, 30)]);
        process(&mut sprite);
        assert_eq!((sprite.images[0].x, sprite.images[0].y), (0, 0));
        assert_eq!(sprite.dimensions(), (50, 30));
    }

    #[test]
    fn test_process_empty_images() {
        let mut sprite = sprite_of(&[]);
        process(&mut sprite);
        assert!(sprite.images.is_empty());
    }

    #[test]
    fn test_process_equal_squares_fill_a_square() {
        let mut sprite = sprite_of(&[(10, 10); 4]);
        process(&mut sprite);
        assert_no_overlap(&sprite);
        assert_eq!(sprite.dimensions(), (20, 20));
    }

    #[test]
    fn test_process_smallest_first_does_not_stack_at_origin() {
        // Review 2.3: with --ordering=-maxside every image ended up at (0,0).
        let mut sprite = sprite_of(&[(4, 4), (100, 4), (4, 100)]);
        process(&mut sprite);
        assert_no_overlap(&sprite);
    }

    #[test]
    fn test_process_tall_after_wide_does_not_overlap() {
        // Review 2.3: with --ordering=width, `tall` was placed over `wide`.
        let mut sprite = sprite_of(&[(100, 4), (4, 4), (4, 100)]);
        process(&mut sprite);
        assert_no_overlap(&sprite);
    }

    #[test]
    fn test_process_first_image_fits_with_spacing() {
        let mut sprite = sprite_of(&[(10, 10), (10, 10)]);
        sprite.padding = crate::util::Spacing::from(Some(String::from("2")));
        process(&mut sprite);
        // Blocks are 14×14: the second one must start after the first block.
        let second = &sprite.images[1];
        assert!(second.x >= 14 || second.y >= 14);
    }

    /// The former recursive search, as a reference.
    fn find_recursive(
        graph: &SquareGraph,
        node_index: NodeIndex,
        width: u32,
        height: u32,
    ) -> Option<NodeIndex> {
        let node = &graph.nodes[node_index];
        if node.used {
            find_recursive(graph, node.right.unwrap(), width, height)
                .or_else(|| find_recursive(graph, node.down.unwrap(), width, height))
        } else if node.width >= width && node.height >= height {
            Some(node_index)
        } else {
            None
        }
    }

    #[test]
    fn test_iterative_find_matches_recursive_search() {
        let mut state: u32 = 0x1234_5678;
        let mut next = move |modulo: u32| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state % modulo
        };
        for _ in 0..50 {
            let blocks: Vec<(u32, u32)> = (0..40).map(|_| (1 + next(30), 1 + next(30))).collect();
            let mut graph = SquareGraph::default();
            let root = graph.add_node(SquareNode {
                width: blocks.iter().map(|block| block.0).max().unwrap(),
                height: blocks.iter().map(|block| block.1).max().unwrap(),
                ..Default::default()
            });
            for &(width, height) in &blocks {
                // Probe other sizes too, so that failed searches are compared as well.
                for (probe_w, probe_h) in [(width, height), (1 + next(40), 1 + next(40))] {
                    assert_eq!(
                        graph.find(root, probe_w, probe_h),
                        find_recursive(&graph, root, probe_w, probe_h)
                    );
                }
                match graph.find(root, width, height) {
                    Some(free) => graph.split(free, width, height),
                    None => {
                        graph.grow(root, width, height);
                    }
                }
            }
        }
    }

    /// Depth of the deepest node below `root`, measured without recursion.
    fn tree_depth(graph: &SquareGraph, root: NodeIndex) -> usize {
        let mut deepest = 0;
        let mut pending = vec![(root, 1)];
        while let Some((index, depth)) = pending.pop() {
            deepest = deepest.max(depth);
            let node = &graph.nodes[index];
            for child in [node.right, node.down].into_iter().flatten() {
                pending.push((child, depth + 1));
            }
        }
        deepest
    }

    /// Pack `blocks` like [`process`] and return the graph.
    fn pack(blocks: &[(u32, u32)]) -> SquareGraph {
        let mut graph = SquareGraph::default();
        let root = graph.add_node(SquareNode {
            width: blocks.iter().map(|block| block.0).max().unwrap(),
            height: blocks.iter().map(|block| block.1).max().unwrap(),
            ..Default::default()
        });
        for &(width, height) in blocks {
            match graph.find(root, width, height) {
                Some(free) => graph.split(free, width, height),
                None => {
                    graph.grow(root, width, height);
                }
            }
        }
        graph
    }

    #[test]
    fn test_deep_tree_does_not_overflow_the_stack() {
        // Review 5.5: the tree can be as deep as a column of images; the recursive search
        // overflowed small (e.g. rayon) thread stacks.
        // On the heap: a 20 000-element array would not fit the small thread stack itself.
        let blocks = vec![(100, 1); 20_000];
        assert!(
            tree_depth(&pack(&blocks), 0) > 1_000,
            "the test must build a deep tree"
        );
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || {
                let mut sprite = sprite_of(&blocks);
                process(&mut sprite);
                sprite
            })
            .unwrap();
        let sprite = handle.join().expect("packing must not overflow the stack");
        assert_no_overlap(&sprite);
    }

    #[test]
    fn test_free_bounds_match_the_free_nodes() {
        let blocks: Vec<(u32, u32)> = (0..200u32)
            .map(|i| (1 + i * 7 % 23, 1 + i * 5 % 17))
            .collect();
        let graph = pack(&blocks);
        for index in 0..graph.nodes.len() {
            // Recompute the bound from the free leaves of the subtree.
            let mut expected = (0, 0);
            let mut pending = vec![index];
            while let Some(current) = pending.pop() {
                let node = &graph.nodes[current];
                if node.used {
                    pending.extend(node.right);
                    pending.extend(node.down);
                } else {
                    if node.width > 0 && node.height > 0 {
                        expected = (expected.0.max(node.width), expected.1.max(node.height));
                    }
                }
            }
            assert_eq!(graph.free[index], expected, "node {index}");
        }
    }
}
