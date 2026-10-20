use std::cmp::Reverse;
use std::collections::hash_map::Entry;
use std::collections::{BinaryHeap, HashMap};

pub use tracing::{debug, info, warn};
#[inline]
pub fn get_cell_index_of(x: u16, y: u16, x_size: u16) -> usize {
    (x as u32 + y as u32 * x_size as u32) as usize
}
// Coming from herculesWS
pub static MOVE_COST: u16 = 10;
pub static MOVE_DIAGONAL_COST: u16 = 14;

pub static DIR_NORTH: u8 = 1;
pub static DIR_WEST: u8 = 2;
pub static DIR_SOUTH: u8 = 4;
pub static DIR_EAST: u8 = 8;

// Upper bound on expanded cells. Large enough to walk around long walls and
// building blocks, small enough to keep an unreachable destination cheap for the
// game loop.
const MAX_EXPANDED_NODES: usize = 4096;

#[derive(Copy, Clone, Debug)]
pub struct PathNode {
    pub id: u32,
    pub parent_id: u32,
    pub x: u16,
    pub y: u16,
    pub g_cost: u16,
    pub f_cost: u16, // g_cost + heuristic
    pub is_open: bool,
    pub is_diagonal: bool,
}

impl PathNode {
    pub fn set_parent_id(&mut self, parent_id: u32) {
        self.parent_id = parent_id;
    }

    pub fn set_g_cost(&mut self, g_cost: u16) {
        self.g_cost = g_cost;
    }

    pub fn set_f_cost(&mut self, f_cost: u16) {
        self.f_cost = f_cost;
    }

    pub fn set_is_open(&mut self, is_open: bool) {
        self.is_open = is_open;
    }
}

// Octile distance on the same cost scale as the move costs (10 straight, 14 diagonal).
// A Manhattan heuristic is not admissible for 8-way movement and makes A* prefer
// orthogonal steps, which turns every path into a square staircase instead of a
// diagonal one.
#[inline]
fn client_side_heuristic(x0: u16, y0: u16, x1: u16, y1: u16) -> u32 {
    let dx = (x1 as i32 - x0 as i32).unsigned_abs();
    let dy = (y1 as i32 - y0 as i32).unsigned_abs();
    let diagonal = dx.min(dy);
    let straight = dx.max(dy) - diagonal;
    MOVE_DIAGONAL_COST as u32 * diagonal + MOVE_COST as u32 * straight
}

#[inline]
pub fn manhattan_distance(x0: u16, y0: u16, x1: u16, y1: u16) -> u16 {
    i16::abs(x1 as i16 - x0 as i16) as u16 + i16::abs(y1 as i16 - y0 as i16) as u16
}

#[inline]
pub fn is_direction(allowed_dir: u8, direction: u8) -> bool {
    (allowed_dir & direction) == direction
}

#[inline]
fn node_id(x: u16, y: u16, x_size: u16) -> u32 {
    x as u32 + y as u32 * x_size as u32
}

fn is_cell_walkable(cells: &[u16], x: u16, y: u16, x_size: u16) -> bool {
    if cells.is_empty() {
        warn!("Cannot call is_cell_walkable as cells are not initialized, returning false");
        return false;
    }
    let index = get_cell_index_of(x, y, x_size);
    cells.get(index).is_some_and(|cell| (cell & 0b0000_0000_0000_0001) == 0b0000_0000_0000_0001)
}

struct SearchNode {
    x: u16,
    y: u16,
    parent_id: u32,
    g_cost: u32,
    closed: bool,
    is_diagonal: bool,
}

/**
 * Client use a A* algorithm for path finding.
 * Returns the steps from the cell after the source up to the destination, or an
 * empty path when the destination is unwalkable, unreachable, or farther than the
 * search budget allows.
 */
pub fn path_search_client_side_algorithm(
    x_size: u16,
    y_size: u16,
    cells: &[u16],
    source_x: u16,
    source_y: u16,
    destination_x: u16,
    destination_y: u16,
) -> Vec<PathNode> {
    if x_size == 0
        || y_size == 0
        || source_x >= x_size
        || source_y >= y_size
        || destination_x >= x_size
        || destination_y >= y_size
        || !is_cell_walkable(cells, destination_x, destination_y, x_size)
    {
        return vec![];
    }
    let max_x = x_size - 1;
    let max_y = y_size - 1;
    let source_id = node_id(source_x, source_y, x_size);
    let destination_id = node_id(destination_x, destination_y, x_size);

    let mut nodes: HashMap<u32, SearchNode> = HashMap::with_capacity(256);
    nodes.insert(
        source_id,
        SearchNode {
            x: source_x,
            y: source_y,
            parent_id: source_id,
            g_cost: 0,
            closed: false,
            is_diagonal: false,
        },
    );
    // Ties on f are broken toward the lowest heuristic so the search keeps pushing
    // toward the destination instead of widening the frontier.
    let mut open_set: BinaryHeap<Reverse<(u32, u32, u32)>> = BinaryHeap::with_capacity(256);
    let source_h = client_side_heuristic(source_x, source_y, destination_x, destination_y);
    open_set.push(Reverse((source_h, source_h, source_id)));

    let neighbors: [(i32, i32, u8, bool); 8] = [
        (1, -1, DIR_SOUTH | DIR_EAST, true),
        (1, 0, DIR_EAST, false),
        (1, 1, DIR_NORTH | DIR_EAST, true),
        (0, 1, DIR_NORTH, false),
        (-1, 1, DIR_NORTH | DIR_WEST, true),
        (-1, 0, DIR_WEST, false),
        (-1, -1, DIR_SOUTH | DIR_WEST, true),
        (0, -1, DIR_SOUTH, false),
    ];

    let mut expanded = 0;
    let mut found = false;
    while let Some(Reverse((f_cost, _, current_id))) = open_set.pop() {
        let current = nodes.get_mut(&current_id).unwrap();
        // Stale heap entry: the node was closed or reached later with a lower cost.
        if current.closed || f_cost != current.g_cost + client_side_heuristic(current.x, current.y, destination_x, destination_y) {
            continue;
        }
        if current_id == destination_id {
            found = true;
            break;
        }
        if expanded >= MAX_EXPANDED_NODES {
            break;
        }
        expanded += 1;
        current.closed = true;
        let (current_x, current_y, current_g) = (current.x, current.y, current.g_cost);
        let allowed_dirs = allowed_dirs(max_x, max_y, current_x, current_y);
        for (dx, dy, direction, is_diagonal) in neighbors {
            if !is_direction(allowed_dirs, direction) {
                continue;
            }
            let x = (current_x as i32 + dx) as u16;
            let y = (current_y as i32 + dy) as u16;
            if !is_cell_walkable(cells, x, y, x_size) {
                continue;
            }
            let move_cost = if is_diagonal { MOVE_DIAGONAL_COST } else { MOVE_COST } as u32;
            let tentative_g_cost = current_g + move_cost;
            let id = node_id(x, y, x_size);
            match nodes.entry(id) {
                Entry::Occupied(mut entry) => {
                    let neighbor = entry.get_mut();
                    if neighbor.closed || tentative_g_cost >= neighbor.g_cost {
                        continue;
                    }
                    neighbor.parent_id = current_id;
                    neighbor.g_cost = tentative_g_cost;
                    neighbor.is_diagonal = is_diagonal;
                }
                Entry::Vacant(entry) => {
                    entry.insert(SearchNode {
                        x,
                        y,
                        parent_id: current_id,
                        g_cost: tentative_g_cost,
                        closed: false,
                        is_diagonal,
                    });
                }
            }
            let h_cost = client_side_heuristic(x, y, destination_x, destination_y);
            open_set.push(Reverse((tentative_g_cost + h_cost, h_cost, id)));
        }
    }

    if !found {
        debug!(
            "No path from {},{} to {},{} (expanded {} nodes)",
            source_x, source_y, destination_x, destination_y, expanded
        );
        return vec![];
    }

    let mut final_path: Vec<PathNode> = Vec::with_capacity(32);
    let mut current_id = destination_id;
    while current_id != source_id {
        let node = &nodes[&current_id];
        let g_cost = node.g_cost.min(u16::MAX as u32) as u16;
        final_path.push(PathNode {
            id: current_id,
            parent_id: node.parent_id,
            x: node.x,
            y: node.y,
            g_cost,
            f_cost: g_cost,
            is_open: false,
            is_diagonal: node.is_diagonal,
        });
        current_id = node.parent_id;
    }
    final_path.reverse();
    final_path
}

pub fn allowed_dirs(max_x: u16, max_y: u16, x: u16, y: u16) -> u8 {
    let mut allowed_dirs: u8 = 0;
    if y < max_y {
        allowed_dirs |= DIR_NORTH;
    }
    if y > 0 {
        allowed_dirs |= DIR_SOUTH;
    }
    if x < max_x {
        allowed_dirs |= DIR_EAST;
    }
    if x > 0 {
        allowed_dirs |= DIR_WEST;
    }
    allowed_dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALKABLE: u16 = 1;
    const BLOCKED: u16 = 0;

    fn open_map(x_size: u16, y_size: u16) -> Vec<u16> {
        vec![WALKABLE; x_size as usize * y_size as usize]
    }

    fn block(cells: &mut [u16], x_size: u16, x: u16, y: u16) {
        cells[get_cell_index_of(x, y, x_size)] = BLOCKED;
    }

    fn assert_valid_path(cells: &[u16], x_size: u16, source: (u16, u16), path: &[PathNode]) {
        let mut previous = source;
        for node in path {
            let dx = (node.x as i32 - previous.0 as i32).abs();
            let dy = (node.y as i32 - previous.1 as i32).abs();
            assert!(dx <= 1 && dy <= 1 && dx + dy > 0, "non adjacent step {previous:?} -> {},{}", node.x, node.y);
            assert_eq!(node.is_diagonal, dx == 1 && dy == 1);
            assert!(is_cell_walkable(cells, node.x, node.y, x_size), "step on blocked cell {},{}", node.x, node.y);
            previous = (node.x, node.y);
        }
    }

    #[test]
    fn straight_line_on_open_ground() {
        let cells = open_map(20, 20);
        let path = path_search_client_side_algorithm(20, 20, &cells, 2, 5, 12, 5);
        assert_eq!(path.len(), 10);
        assert_eq!((path.last().unwrap().x, path.last().unwrap().y), (12, 5));
        assert_valid_path(&cells, 20, (2, 5), &path);
    }

    #[test]
    fn same_cell_returns_empty_path() {
        let cells = open_map(10, 10);
        assert!(path_search_client_side_algorithm(10, 10, &cells, 3, 3, 3, 3).is_empty());
    }

    #[test]
    fn walks_around_long_wall() {
        let (x_size, y_size) = (80, 80);
        let mut cells = open_map(x_size, y_size);
        for y in 10..=70 {
            block(&mut cells, x_size, 40, y);
        }
        let path = path_search_client_side_algorithm(x_size, y_size, &cells, 35, 40, 45, 40);
        assert!(!path.is_empty());
        assert_eq!((path.last().unwrap().x, path.last().unwrap().y), (45, 40));
        assert_valid_path(&cells, x_size, (35, 40), &path);
    }

    #[test]
    fn backs_out_of_dead_end() {
        // U shaped pocket opening west, source inside, destination behind its east wall.
        let (x_size, y_size) = (60, 60);
        let mut cells = open_map(x_size, y_size);
        for x in 20..=40 {
            block(&mut cells, x_size, x, 20);
            block(&mut cells, x_size, x, 40);
        }
        for y in 20..=40 {
            block(&mut cells, x_size, 40, y);
        }
        let path = path_search_client_side_algorithm(x_size, y_size, &cells, 35, 30, 45, 30);
        assert!(!path.is_empty());
        assert!(path.iter().any(|node| node.x < 20), "path should leave the pocket through its opening");
        assert_eq!((path.last().unwrap().x, path.last().unwrap().y), (45, 30));
        assert_valid_path(&cells, x_size, (35, 30), &path);
    }

    #[test]
    fn detour_is_shortest() {
        // Wall from y=0 to y=8 at x=5: shortest route goes around its top end.
        let (x_size, y_size) = (12, 12);
        let mut cells = open_map(x_size, y_size);
        for y in 0..=8 {
            block(&mut cells, x_size, 5, y);
        }
        let path = path_search_client_side_algorithm(x_size, y_size, &cells, 3, 4, 7, 4);
        assert_valid_path(&cells, x_size, (3, 4), &path);
        let cost: u16 = path.iter().map(|node| if node.is_diagonal { MOVE_DIAGONAL_COST } else { MOVE_COST }).sum();
        assert_eq!(path.last().unwrap().g_cost, cost);
        // Through (5,9): (3,4)->(5,9) is 2 diagonals + 3 straight = 58, same on the way back down.
        assert_eq!(cost, 116);
    }

    #[test]
    fn unreachable_enclosed_destination_returns_empty() {
        let (x_size, y_size) = (20, 20);
        let mut cells = open_map(x_size, y_size);
        for (dx, dy) in [(-1i32, -1i32), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
            block(&mut cells, x_size, (10 + dx) as u16, (10 + dy) as u16);
        }
        assert!(path_search_client_side_algorithm(x_size, y_size, &cells, 2, 2, 10, 10).is_empty());
    }

    #[test]
    fn source_enclosed_returns_empty() {
        let (x_size, y_size) = (8, 8);
        let mut cells = open_map(x_size, y_size);
        for (dx, dy) in [(-1i32, -1i32), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
            block(&mut cells, x_size, (3 + dx) as u16, (3 + dy) as u16);
        }
        assert!(path_search_client_side_algorithm(x_size, y_size, &cells, 3, 3, 6, 6).is_empty());
    }

    #[test]
    fn unwalkable_or_out_of_bounds_destination_returns_empty() {
        let mut cells = open_map(10, 10);
        block(&mut cells, 10, 5, 5);
        assert!(path_search_client_side_algorithm(10, 10, &cells, 1, 1, 5, 5).is_empty());
        assert!(path_search_client_side_algorithm(10, 10, &cells, 1, 1, 10, 3).is_empty());
    }

    #[test]
    fn node_ids_are_distinct_across_row_boundary() {
        assert_ne!(node_id(9, 0, 10), node_id(0, 1, 10));
        let cells = open_map(10, 10);
        let path = path_search_client_side_algorithm(10, 10, &cells, 9, 0, 0, 1);
        assert_eq!(path.len(), 9);
        assert_valid_path(&cells, 10, (9, 0), &path);
    }
}
