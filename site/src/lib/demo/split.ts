import { type Rect, rect } from '../term/grid';

export type Dir = 'right' | 'down';

export type Node = { leaf: number } | { dir: Dir; ratio: number; first: Node; second: Node };

export interface Divider {
  path: boolean[];
  dir: Dir;
  area: Rect;
  line: Rect;
}

const PADDING = 1;
const MIN_COLS = 10;
const MIN_ROWS = 3;

const span = (area: Rect, dir: Dir): [number, number] =>
  dir === 'right' ? [area.w, Math.min(1 + PADDING, area.w)] : [area.h, Math.min(1, area.h)];

function firstSize(room: number, ratio: number): number {
  if (room < 2) return room;
  return Math.max(1, Math.min(room - 1, Math.round(room * Math.max(0, Math.min(1, ratio)))));
}

export function splitRect(area: Rect, dir: Dir, ratio: number): [Rect, Rect, Rect] {
  const [total, gap] = span(area, dir);
  const room = total - gap;
  const first = firstSize(room, ratio);
  const second = room - first;
  if (dir === 'right') {
    return [
      rect(area.x, area.y, first, area.h),
      rect(area.x + first + gap, area.y, second, area.h),
      rect(area.x + first, area.y, Math.min(gap, 1), area.h),
    ];
  }
  return [
    rect(area.x, area.y, area.w, first),
    rect(area.x, area.y + first + gap, area.w, second),
    rect(area.x, area.y + first, area.w, gap),
  ];
}

export function fits(area: Rect, dir: Dir): boolean {
  const [total, gap] = span(area, dir);
  return total - gap >= (dir === 'right' ? MIN_COLS : MIN_ROWS) * 2;
}

export function ids(node: Node): number[] {
  return 'leaf' in node ? [node.leaf] : [...ids(node.first), ...ids(node.second)];
}

export function panes(node: Node, area: Rect): [number, Rect][] {
  if ('leaf' in node) return [[node.leaf, area]];
  const [a, b] = splitRect(area, node.dir, node.ratio);
  return [...panes(node.first, a), ...panes(node.second, b)];
}

export function dividers(node: Node, area: Rect, path: boolean[] = []): Divider[] {
  if ('leaf' in node) return [];
  const [a, b, line] = splitRect(area, node.dir, node.ratio);
  return [
    { path, dir: node.dir, area, line },
    ...dividers(node.first, a, [...path, false]),
    ...dividers(node.second, b, [...path, true]),
  ];
}

export const grab = (d: Divider): Rect =>
  d.dir === 'right' ? rect(d.line.x, d.line.y, Math.min(d.line.w + PADDING, d.area.x + d.area.w - d.line.x), d.line.h) : d.line;

export function ratioAt(d: Divider, x: number, y: number): number {
  const [start, at] = d.dir === 'right' ? [d.area.x, x] : [d.area.y, y];
  const [total, gap] = span(d.area, d.dir);
  const room = total - gap;
  if (room < 2) return 0.5;
  return Math.max(1, Math.min(room - 1, at - start)) / room;
}

export function split(node: Node, target: number, dir: Dir, fresh: number): Node {
  if ('leaf' in node) {
    return node.leaf === target ? { dir, ratio: 0.5, first: node, second: { leaf: fresh } } : node;
  }
  return { ...node, first: split(node.first, target, dir, fresh), second: split(node.second, target, dir, fresh) };
}

export function remove(node: Node, target: number): Node | null {
  if ('leaf' in node) return node.leaf === target ? null : node;
  const first = remove(node.first, target);
  const second = remove(node.second, target);
  if (!first) return second;
  if (!second) return first;
  return { ...node, first, second };
}

export function setRatio(node: Node, path: boolean[], value: number): Node {
  if ('leaf' in node) return node;
  if (path.length === 0) return { ...node, ratio: value };
  const [head, ...rest] = path;
  return head ? { ...node, second: setRatio(node.second, rest, value) } : { ...node, first: setRatio(node.first, rest, value) };
}
