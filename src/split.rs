use ratatui::layout::{Position, Rect};
use serde::{Deserialize, Serialize};

pub const HALF: f32 = 0.5;
pub const MIN_COLS: u16 = 10;
pub const MIN_ROWS: u16 = 3;
const PADDING: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Right,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node<T> {
    Leaf(T),
    Split { dir: Dir, ratio: f32, first: Box<Node<T>>, second: Box<Node<T>> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    pub path: Vec<bool>,
    pub dir: Dir,
    pub area: Rect,
    pub line: Rect,
}

fn span(area: Rect, dir: Dir) -> (u16, u16) {
    match dir {
        Dir::Right => (area.width, (1 + PADDING).min(area.width)),
        Dir::Down => (area.height, area.height.min(1)),
    }
}

impl Divider {
    pub fn grab(&self) -> Rect {
        match self.dir {
            Dir::Right => Rect { width: self.line.width + PADDING, ..self.line }.intersection(self.area),
            Dir::Down => self.line,
        }
    }
}

pub fn split_rect(area: Rect, dir: Dir, ratio: f32) -> (Rect, Rect, Rect) {
    let (total, gap) = span(area, dir);
    let room = total - gap;
    let first = first_size(room, ratio);
    let second = room - first;
    match dir {
        Dir::Right => (
            Rect { width: first, ..area },
            Rect { x: area.x + first + gap, width: second, ..area },
            Rect { x: area.x + first, width: gap.min(1), ..area },
        ),
        Dir::Down => (
            Rect { height: first, ..area },
            Rect { y: area.y + first + gap, height: second, ..area },
            Rect { y: area.y + first, height: gap, ..area },
        ),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the ratio is clamped to 0..=1, so the product fits in the u16 it came from"
)]
fn first_size(room: u16, ratio: f32) -> u16 {
    if room < 2 {
        return room;
    }
    let size = (f32::from(room) * ratio.clamp(0.0, 1.0)).round() as u16;
    size.clamp(1, room - 1)
}

pub fn ratio_at(divider: &Divider, pos: Position) -> f32 {
    let (start, at) = match divider.dir {
        Dir::Right => (divider.area.x, pos.x),
        Dir::Down => (divider.area.y, pos.y),
    };
    let (total, gap) = span(divider.area, divider.dir);
    let room = total - gap;
    if room < 2 {
        return HALF;
    }
    let first = at.saturating_sub(start).clamp(1, room - 1);
    f32::from(first) / f32::from(room)
}

pub fn fits(area: Rect, dir: Dir) -> bool {
    let (total, gap) = span(area, dir);
    let min = match dir {
        Dir::Right => MIN_COLS,
        Dir::Down => MIN_ROWS,
    };
    total - gap >= min * 2
}

impl<T: Copy + PartialEq> Node<T> {
    pub fn ids(&self) -> Vec<T> {
        let mut ids = Vec::new();
        self.collect_ids(&mut ids);
        ids
    }

    fn collect_ids(&self, ids: &mut Vec<T>) {
        match self {
            Self::Leaf(id) => ids.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_ids(ids);
                second.collect_ids(ids);
            }
        }
    }

    pub fn panes(&self, area: Rect) -> Vec<(T, Rect)> {
        let mut panes = Vec::new();
        self.collect_panes(area, &mut panes);
        panes
    }

    fn collect_panes(&self, area: Rect, panes: &mut Vec<(T, Rect)>) {
        match self {
            Self::Leaf(id) => panes.push((*id, area)),
            Self::Split { dir, ratio, first, second } => {
                let (a, b, _) = split_rect(area, *dir, *ratio);
                first.collect_panes(a, panes);
                second.collect_panes(b, panes);
            }
        }
    }

    pub fn pane(&self, area: Rect, id: T) -> Option<Rect> {
        self.panes(area).into_iter().find(|(p, _)| *p == id).map(|(_, r)| r)
    }

    pub fn pane_at(&self, area: Rect, pos: Position) -> Option<T> {
        self.panes(area).into_iter().find(|(_, r)| r.contains(pos)).map(|(id, _)| id)
    }

    pub fn dividers(&self, area: Rect) -> Vec<Divider> {
        let mut dividers = Vec::new();
        self.collect_dividers(area, &mut Vec::new(), &mut dividers);
        dividers
    }

    fn collect_dividers(&self, area: Rect, path: &mut Vec<bool>, dividers: &mut Vec<Divider>) {
        let Self::Split { dir, ratio, first, second } = self else { return };
        let (a, b, line) = split_rect(area, *dir, *ratio);
        dividers.push(Divider { path: path.clone(), dir: *dir, area, line });
        path.push(false);
        first.collect_dividers(a, path, dividers);
        path.pop();
        path.push(true);
        second.collect_dividers(b, path, dividers);
        path.pop();
    }

    pub fn divider_at(&self, area: Rect, pos: Position) -> Option<Divider> {
        self.dividers(area).into_iter().find(|d| d.grab().contains(pos))
    }

    pub fn split(&mut self, target: T, dir: Dir, new: T) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                *self = Self::Split {
                    dir,
                    ratio: HALF,
                    first: Box::new(Self::Leaf(target)),
                    second: Box::new(Self::Leaf(new)),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => first.split(target, dir, new) || second.split(target, dir, new),
        }
    }

    pub fn remove(&mut self, target: T) -> Option<T> {
        let Self::Split { first, second, .. } = self else { return None };
        let (kept, neighbour) = if **first == Self::Leaf(target) {
            (std::mem::replace(second.as_mut(), Self::Leaf(target)), true)
        } else if **second == Self::Leaf(target) {
            (std::mem::replace(first.as_mut(), Self::Leaf(target)), false)
        } else {
            return first.remove(target).or_else(|| second.remove(target));
        };
        let ids = kept.ids();
        let near = if neighbour { ids.first() } else { ids.last() }.copied();
        *self = kept;
        near
    }

    pub fn set_ratio(&mut self, path: &[bool], value: f32) -> bool {
        match (self, path.split_first()) {
            (Self::Split { ratio, .. }, None) => {
                *ratio = value;
                true
            }
            (Self::Split { first, second, .. }, Some((&go_second, rest))) => {
                if go_second {
                    second.set_ratio(rest, value)
                } else {
                    first.set_ratio(rest, value)
                }
            }
            (Self::Leaf(_), _) => false,
        }
    }

    pub fn map<U>(&self, f: &impl Fn(T) -> Option<U>) -> Option<Node<U>> {
        Some(match self {
            Self::Leaf(id) => Node::Leaf(f(*id)?),
            Self::Split { dir, ratio, first, second } => Node::Split {
                dir: *dir,
                ratio: *ratio,
                first: Box::new(first.map(f)?),
                second: Box::new(second.map(f)?),
            },
        })
    }
}

pub fn row<T: Copy>(ids: &[T]) -> Option<Node<T>> {
    let (last, rest) = ids.split_last()?;
    let mut node = Node::Leaf(*last);
    for (i, id) in rest.iter().enumerate().rev() {
        #[expect(clippy::cast_precision_loss, reason = "a tab never holds millions of panes")]
        let ratio = 1.0 / (ids.len() - i) as f32;
        node = Node::Split { dir: Dir::Right, ratio, first: Box::new(Node::Leaf(*id)), second: Box::new(node) };
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const AREA: Rect = Rect { x: 10, y: 2, width: 42, height: 21 };

    fn pair(dir: Dir) -> Node<u64> {
        let mut node = Node::Leaf(1);
        node.split(1, dir, 2);
        node
    }

    fn three() -> Node<u64> {
        let mut node = pair(Dir::Right);
        node.split(2, Dir::Down, 3);
        node
    }

    mod geometry {
        use super::*;

        #[test]
        fn a_leaf_takes_the_whole_area() {
            assert_eq!(Node::Leaf(7).panes(AREA), vec![(7, AREA)]);
        }

        #[test]
        fn split_right_leaves_the_divider_and_a_blank_column_between_the_halves() {
            let panes = pair(Dir::Right).panes(AREA);
            assert_eq!(panes, vec![(1, Rect::new(10, 2, 20, 21)), (2, Rect::new(32, 2, 20, 21))]);
        }

        #[test]
        fn split_down_leaves_one_row_between_the_halves() {
            let panes = pair(Dir::Down).panes(AREA);
            assert_eq!(panes, vec![(1, Rect::new(10, 2, 42, 10)), (2, Rect::new(10, 13, 42, 10))]);
        }

        #[test]
        fn the_divider_sits_between_the_halves() {
            let dividers = pair(Dir::Right).dividers(AREA);
            assert_eq!(dividers.iter().map(|d| d.line).collect::<Vec<_>>(), vec![Rect::new(30, 2, 1, 21)]);
        }

        #[test]
        fn nested_dividers_carry_their_path() {
            let paths: Vec<Vec<bool>> = three().dividers(AREA).into_iter().map(|d| d.path).collect();
            assert_eq!(paths, vec![vec![], vec![true]]);
        }

        #[test]
        fn halves_never_shrink_to_nothing() {
            let mut node = pair(Dir::Right);
            node.set_ratio(&[], 0.0);
            assert_eq!(node.panes(AREA)[0].1.width, 1);
        }

        #[test]
        fn a_pane_is_found_under_the_mouse() {
            assert_eq!(three().pane_at(AREA, Position::new(40, 20)), Some(3));
        }

        #[test]
        fn the_divider_is_not_a_pane() {
            assert_eq!(pair(Dir::Right).pane_at(AREA, Position::new(30, 5)), None);
        }

        #[test]
        fn the_blank_column_after_the_divider_also_grabs_it() {
            assert_eq!(pair(Dir::Right).divider_at(AREA, Position::new(31, 5)).map(|d| d.path), Some(vec![]));
        }
    }

    mod editing {
        use super::*;

        #[test]
        fn removing_a_pane_gives_its_space_to_the_sibling() {
            let mut node = three();
            node.remove(3);
            assert_eq!(node, pair(Dir::Right));
        }

        #[test]
        fn removing_the_first_half_keeps_the_second() {
            let mut node = pair(Dir::Right);
            node.remove(1);
            assert_eq!(node, Node::Leaf(2));
        }

        #[rstest]
        #[case::the_first_goes_to_the_nearest_of_the_rest(1, Some(2))]
        #[case::the_last_goes_to_its_sibling(3, Some(2))]
        #[case::a_missing_one_changes_nothing(9, None)]
        fn removing_names_the_pane_that_takes_the_space(#[case] removed: u64, #[case] near: Option<u64>) {
            assert_eq!(three().remove(removed), near);
        }

        #[test]
        fn the_ratio_of_a_nested_split_changes_by_path() {
            let mut node = three();
            node.set_ratio(&[true], 0.25);
            let Node::Split { second, .. } = node else { panic!("expected a split") };
            assert!(matches!(*second, Node::Split { ratio, .. } if (ratio - 0.25).abs() < f32::EPSILON));
        }

        #[test]
        fn dragging_puts_the_divider_under_the_mouse() {
            let mut node = pair(Dir::Right);
            let divider = node.dividers(AREA).remove(0);
            node.set_ratio(&divider.path, ratio_at(&divider, Position::new(20, 5)));
            assert_eq!(node.dividers(AREA)[0].line.x, 20);
        }

        #[test]
        fn a_row_shares_the_width() {
            let widths: Vec<u16> = row(&[1, 2, 3]).expect("a row").panes(AREA).iter().map(|(_, r)| r.width).collect();
            assert_eq!(widths, vec![13, 13, 12]);
        }

        #[test]
        fn map_drops_the_tree_when_an_id_is_unknown() {
            assert_eq!(three().map(&|id| (id != 3).then_some(id)), None);
        }

        #[rstest]
        #[case::wide_enough_for_right(Rect::new(0, 0, 22, 5), Dir::Right, true)]
        #[case::too_narrow_for_right(Rect::new(0, 0, 21, 50), Dir::Right, false)]
        #[case::tall_enough_for_down(Rect::new(0, 0, 5, 7), Dir::Down, true)]
        #[case::too_short_for_down(Rect::new(0, 0, 80, 6), Dir::Down, false)]
        fn a_split_needs_room_for_both_halves(#[case] area: Rect, #[case] dir: Dir, #[case] expected: bool) {
            assert_eq!(fits(area, dir), expected);
        }
    }
}
