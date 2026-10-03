import { type Rect, rect } from '../term/grid';

export const SIDEBAR_WIDTH = 32;
export const WORKSPACES_WIDTH = 26;
export const MIN_COLUMN_WIDTH = 16;
export const COMPACT_WIDTH = 90;
const PANE_PADDING = 1;
const MIN_PANE_WIDTH = 20;
const COMPACT_PITCH = 3;
const COMPACT_BUTTON_WIDTH = 7;
const HEADER_HEIGHT = 5;
export const GAP = 1;
const FORM_WIDTH = 64;
const FORM_HEIGHT = 10;
const PICKER_WIDTH = 72;
const PICKER_HEIGHT = 24;
const ISSUES_WIDTH = 110;
const ISSUES_HEIGHT = 30;

export const GROUP_ICONS = ['●', '◉', '◐', '◆', '■', '▲', '▼', '★', '✦', '♥', '♣', '♠'];
export const GROUP_COLOURS = [1, 9, 208, 214, 3, 11, 2, 10, 6, 14, 4, 12, 99, 5, 13, 205];
export const STYLE_DONE = 'done';
const ICONS_PER_ROW = 6;
const COLOURS_PER_ROW = 8;
const ICON_CELL = 3;
const COLOUR_CELL = 4;

export type Nav = 'projects' | 'workspaces' | null;

export type SidebarRow = { kind: 'gap' } | { kind: 'group'; g: number } | { kind: 'project'; p: number };

export function sidebarRows(groups: (number | null)[], collapsed: boolean[]): SidebarRow[] {
  const inGroup = (g: number | null): SidebarRow[] =>
    groups.flatMap((group, p) => (group === g ? [{ kind: 'project', p } as SidebarRow] : []));
  const rows = inGroup(null);
  collapsed.forEach((folded, g) => {
    if (rows.length) rows.push({ kind: 'gap' });
    rows.push({ kind: 'group', g });
    if (!folded) rows.push(...inGroup(g));
  });
  return rows;
}

export interface Widths {
  projects: number;
  workspaces: number;
}

export const EMPTY: Rect = rect(0, 0, 0, 0);
export const isEmpty = (r: Rect) => r.w === 0 || r.h === 0;
export const bottom = (r: Rect) => r.y + r.h;
export const right = (r: Rect) => r.x + r.w;

export function intersect(a: Rect, b: Rect): Rect {
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  const w = Math.min(right(a), right(b)) - x;
  const h = Math.min(bottom(a), bottom(b)) - y;
  return w > 0 && h > 0 ? rect(x, y, w, h) : EMPTY;
}

export function fit(w: Widths, total: number): Widths {
  const room = Math.max(0, total - (PANE_PADDING + MIN_PANE_WIDTH));
  const workspaces = Math.max(MIN_COLUMN_WIDTH, Math.min(w.workspaces, room - w.projects));
  const projects = Math.max(MIN_COLUMN_WIDTH, Math.min(w.projects, room - workspaces));
  return { projects, workspaces };
}

export function dragged(w: Widths, border: 'projects' | 'workspaces', x: number, total: number): Widths {
  const fitted = fit(w, total);
  const room = Math.max(0, total - (PANE_PADDING + MIN_PANE_WIDTH));
  const edge = x + 1;
  if (border === 'projects') {
    const max = Math.max(MIN_COLUMN_WIDTH, room - fitted.workspaces);
    return { ...w, projects: Math.max(MIN_COLUMN_WIDTH, Math.min(max, edge)) };
  }
  const max = Math.max(MIN_COLUMN_WIDTH, room - fitted.projects);
  return { ...w, workspaces: Math.max(MIN_COLUMN_WIDTH, Math.min(max, edge - fitted.projects)) };
}

export interface Areas {
  compact: boolean;
  pitch: number;
  bar: Rect;
  brand: Rect;
  search: Rect;
  searchButton: Rect;
  back: Rect;
  sidebar: Rect;
  title: Rect;
  list: Rect;
  separator: Rect;
  settings: Rect;
  quit: Rect;
  workspaces: Rect;
  workspacesTitle: Rect;
  workspacesList: Rect;
  workspacesSeparator: Rect;
  issues: Rect;
  results: Rect;
  pane: Rect;
  projectsBorder: Rect;
  workspacesBorder: Rect;
}

function column(r: Rect, lead: number): [Rect, Rect, Rect, Rect, Rect] {
  const title = rect(r.x, r.y, r.w, Math.min(lead, r.h));
  const listY = r.y + lead + GAP;
  const listH = Math.max(1, r.h - lead - GAP - 3);
  const list = rect(r.x, listY, r.w, listH);
  const separator = rect(r.x, bottom(list), r.w, 1);
  const a = rect(r.x, bottom(list) + 1, r.w, 1);
  const b = rect(r.x, bottom(list) + 2, r.w, 1);
  return [title, list, separator, a, b];
}

export function layout(cols: number, rows: number, widths: Widths, nav: Nav): Areas {
  const areas = cols < COMPACT_WIDTH ? compact(cols, rows) : wide(cols, rows, widths);
  if (!areas.compact) return areas;
  const projects = nav === 'projects' ? areas : { ...areas, sidebar: EMPTY, title: EMPTY, list: EMPTY, separator: EMPTY, settings: EMPTY, quit: EMPTY };
  return nav === 'workspaces'
    ? projects
    : { ...projects, workspaces: EMPTY, workspacesTitle: EMPTY, workspacesList: EMPTY, workspacesSeparator: EMPTY, issues: EMPTY, back: EMPTY };
}

function wide(cols: number, rows: number, widths: Widths): Areas {
  const { projects, workspaces } = fit(widths, cols);
  const columnsWidth = projects + workspaces;
  const pane = rect(columnsWidth + PANE_PADDING, 0, Math.max(1, cols - columnsWidth - PANE_PADDING), rows);
  const header = rect(0, 0, columnsWidth - 1, Math.min(HEADER_HEIGHT, rows));
  const sidebar = rect(0, HEADER_HEIGHT, projects, Math.max(0, rows - HEADER_HEIGHT));
  const [title, list, separator, settings, quit] = column(rect(0, HEADER_HEIGHT, projects - 1, sidebar.h), 1);
  const wsColumn = rect(projects, 0, workspaces, rows);
  const [workspacesTitle, workspacesList, workspacesSeparator, issues] = column(rect(projects, HEADER_HEIGHT, workspaces - 1, sidebar.h), 1);
  return {
    compact: false,
    pitch: 1,
    bar: EMPTY,
    brand: rect(0, 0, header.w, 2),
    search: rect(1, 3, header.w - 2, 1),
    searchButton: rect(1, 3, header.w - 2, 1),
    back: EMPTY,
    sidebar,
    title,
    list,
    separator,
    settings,
    quit,
    workspaces: wsColumn,
    workspacesTitle,
    workspacesList,
    workspacesSeparator,
    issues,
    results: rect(0, HEADER_HEIGHT, columnsWidth - 1, Math.max(0, rows - HEADER_HEIGHT)),
    pane,
    projectsBorder: rect(projects - 1, HEADER_HEIGHT, 1, sidebar.h),
    workspacesBorder: rect(projects + workspaces - 1, 0, 1, rows),
  };
}

function compact(cols: number, rows: number): Areas {
  const pitch = COMPACT_PITCH;
  const bar = rect(0, 0, cols, pitch);
  const below = rect(0, pitch, cols, Math.max(0, rows - pitch));
  const searchWidth = Math.min(COMPACT_BUTTON_WIDTH, cols);
  const menu = rect(0, pitch + GAP, cols, Math.max(0, rows - pitch - GAP));
  const title = rect(0, menu.y, cols, pitch);
  const listY = menu.y + pitch + GAP;
  const list = rect(0, listY, cols, Math.max(1, bottom(menu) - listY - 1 - pitch));
  const separator = rect(0, bottom(list), cols, 1);
  const footer = rect(0, bottom(list) + 1, cols, pitch);
  const half = Math.floor(cols / 2);
  return {
    compact: true,
    pitch,
    bar,
    brand: EMPTY,
    search: bar,
    searchButton: rect(cols - searchWidth, 0, searchWidth, pitch),
    back: intersect(rect(0, title.y, '‹ projects'.length + 2 + 2, pitch), title),
    sidebar: below,
    title,
    list,
    separator,
    settings: rect(0, footer.y, half, pitch),
    quit: rect(half, footer.y, cols - half, pitch),
    workspaces: below,
    workspacesTitle: title,
    workspacesList: list,
    workspacesSeparator: separator,
    issues: footer,
    results: below,
    pane: below,
    projectsBorder: EMPTY,
    workspacesBorder: EMPTY,
  };
}

export class Rows {
  constructor(
    readonly list: Rect,
    readonly heights: number[],
    readonly button: number,
    readonly scroll: number,
  ) {}

  private room(): number {
    return Math.max(0, this.list.h - GAP - this.button);
  }

  private height(a: number, b: number): number {
    let n = 0;
    for (let i = a; i < b; i++) n += this.heights[i];
    return n;
  }

  private fittingBefore(end: number): number {
    const room = this.room();
    let start = end;
    while (start > 0 && this.height(start - 1, end) <= room) start -= 1;
    return start;
  }

  first(): number {
    return Math.min(this.scroll, this.fittingBefore(this.heights.length));
  }

  end(): number {
    const first = this.first();
    const room = this.room();
    let end = first;
    for (let i = first; i < this.heights.length; i++) {
      if (this.height(first, i + 1) <= room) end = i + 1;
      else break;
    }
    return end;
  }

  item(i: number): Rect {
    const first = this.first();
    if (i < first || i >= this.end()) return EMPTY;
    return rect(this.list.x, this.list.y + this.height(first, i), this.list.w, this.heights[i]);
  }

  hidden(): [number, number] {
    return [this.first(), this.heights.length - this.end()];
  }

  scrolled(delta: number): number {
    const max = this.fittingBefore(this.heights.length);
    return Math.max(0, Math.min(max, Math.min(this.scroll, max) + delta));
  }

  reveal(i: number): number {
    const first = this.first();
    if (i >= this.heights.length || (i >= first && i < this.end())) return first;
    if (i < first) return i;
    return Math.min(this.fittingBefore(i + 1), i);
  }

  buttonRect(): Rect {
    const gap = this.heights.length ? GAP : 0;
    const below = this.list.y + this.height(0, this.heights.length) + gap;
    const last = bottom(this.list) - this.button;
    return rect(this.list.x, Math.min(below, last), this.list.w, this.button);
  }

  moreBelow(): Rect {
    return intersect(rect(this.list.x, this.list.y + this.room(), this.list.w, 1), this.list);
  }
}

export const moreAbove = (list: Rect): Rect => (list.y < GAP ? EMPTY : rect(list.x, list.y - GAP, list.w, 1));

export const closeButton = (row: Rect): Rect => {
  const w = row.h > 1 ? 5 : 3;
  return rect(right(row) - w, row.y, Math.min(w, row.w), row.h);
};

export function centered(cols: number, rows: number, w: number, h: number): Rect {
  return rect(Math.floor((cols - w) / 2), Math.floor((rows - h) / 2), w, h);
}

export const formArea = (cols: number, rows: number) => centered(cols, rows, Math.min(Math.max(0, cols - 4), FORM_WIDTH), Math.min(FORM_HEIGHT, rows));
export const pickerArea = (cols: number, rows: number) =>
  centered(cols, rows, Math.min(Math.max(0, cols - 4), PICKER_WIDTH), Math.min(Math.max(0, rows - 2), PICKER_HEIGHT));
export const issuesArea = (cols: number, rows: number) =>
  centered(cols, rows, Math.min(Math.max(0, cols - 4), ISSUES_WIDTH), Math.min(Math.max(0, rows - 2), ISSUES_HEIGHT));

export const inner = (r: Rect): Rect => rect(r.x + 2, r.y + 1, Math.max(0, r.w - 4), Math.max(0, r.h - 2));

export function rightAligned(row: Rect, labels: string[], width: (l: string) => number, gap: number): Rect[] {
  let x = right(row);
  const rects = [...labels].reverse().map((label) => {
    x -= width(label);
    const r = intersect(rect(x, row.y, width(label), 1), row);
    x -= gap;
    return r;
  });
  return rects.reverse();
}

export const buttonWidth = (label: string) => [...label].length + 2;

export function tabsIn(row: Rect, names: string[]): Rect[] {
  let x = row.x;
  return names.map((name) => {
    const r = intersect(rect(x, row.y, buttonWidth(name), 1), row);
    x += buttonWidth(name) + 1;
    return r;
  });
}

export function menuArea(cols: number, rows: number, at: { x: number; y: number }, items: string[]): Rect {
  const longest = Math.max(0, ...items.map((i) => [...i].length));
  const w = Math.min(longest + 4, cols);
  const h = Math.min(items.length + 2, rows);
  return rect(Math.min(at.x, cols - w), Math.min(at.y + 1, rows - h), w, h);
}

function styleRows(cols: number, rows: number): [Rect, Rect, Rect, Rect, Rect] {
  const c = inner(formArea(cols, rows));
  const at = (dy: number, h: number) => intersect(rect(c.x, c.y + dy, c.w, h), c);
  return [at(0, 1), at(1, 2), at(3, 1), at(4, 2), intersect(rect(c.x, bottom(c) - 1, c.w, 1), c)];
}

function gridCell(grid: Rect, perRow: number, width: number, i: number): Rect {
  return intersect(rect(grid.x + (i % perRow) * width, grid.y + Math.floor(i / perRow), width, 1), grid);
}

export const styleLabels = (cols: number, rows: number): [Rect, Rect, Rect] => {
  const [icon, , colour, , last] = styleRows(cols, rows);
  return [icon, colour, last];
};
export const styleIcon = (cols: number, rows: number, i: number) => gridCell(styleRows(cols, rows)[1], ICONS_PER_ROW, ICON_CELL, i);
export const styleColour = (cols: number, rows: number, i: number) => gridCell(styleRows(cols, rows)[3], COLOURS_PER_ROW, COLOUR_CELL, i);

export function styleDone(cols: number, rows: number): Rect {
  const last = styleRows(cols, rows)[4];
  const w = Math.min(buttonWidth(STYLE_DONE), last.w);
  return rect(right(last) - w, last.y, w, last.h);
}
