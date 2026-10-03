import type { Tree } from './data';
import type { Place, Shell } from './programs';
import type { Node } from './split';

export interface Pos {
  x: number;
  y: number;
}

export interface Pane {
  id: number;
  shell: Shell;
  rightClicks: boolean;
}

export interface Tab {
  id: number;
  name?: string;
  layout: Node;
  panes: Pane[];
  active: number;
}

export interface Workspace {
  id: number;
  name?: string;
  branch?: string;
  root: string;
  worktree: boolean;
  tabs: Tab[];
  active: number;
  flags: Place['flags'];
}

export interface Project {
  id: number;
  name?: string;
  folder: string;
  root: string;
  repo: boolean;
  tree: Tree;
  workspaces: Workspace[];
  active: number;
}

export type Target =
  | { kind: 'project'; project: number }
  | { kind: 'workspace'; project: number; workspace: number }
  | { kind: 'tab'; project: number; workspace: number; tab: number };

export type PaneAction = 'split right' | 'split down' | 'send right-clicks to the pane' | 'use this menu on right-click' | 'close pane';

export interface PickItem {
  value: string;
  note: string;
  dangerous?: boolean;
}

export interface SettingsOverlay {
  kind: 'settings';
  page: number;
  cursor: number;
  pick?: { row: string; title: string; items: PickItem[]; selected: number; filter: string };
  edit?: { row: string; label: string; input: string; token: boolean; error?: string };
  notice?: string;
  busy?: string;
}

export interface IssuesOverlay {
  kind: 'issues';
  project: number;
  tab: number;
  closed: boolean;
  mine: boolean;
  filter: string;
  selected: number;
  scroll: number;
  detail: string | null;
  raw: boolean;
  detailScroll: number;
  loading: boolean;
  busy?: string;
  notice?: string;
  token: { input: string; checking: boolean; error?: string };
  agentPick: { selected: number; filter: string } | null;
  chosen: string | null;
}

export type Overlay =
  | { kind: 'menu'; at: Pos; target: Target }
  | { kind: 'paneMenu'; at: Pos; pane: number; actions: PaneAction[] }
  | { kind: 'newWorkspace'; project: number; input: string; worktree: boolean | null; error?: string; creating?: boolean }
  | { kind: 'rename'; target: Target; input: string }
  | { kind: 'remove'; project: number; workspace: number; removing?: boolean }
  | { kind: 'picker'; dir: string[]; filter: string; selected: number | null; scroll: number }
  | { kind: 'search'; query: string; selected: number; scroll: number }
  | SettingsOverlay
  | IssuesOverlay;

export interface Config {
  worktreesDir: string;
  agent: string;
  submit: boolean;
  trust: boolean;
  dim: boolean;
  updates: boolean;
  agentArgs: Record<string, string[]>;
  sources: string[];
  accounts: { shortcut: boolean; linear: boolean };
}

export const defaultConfig = (): Config => ({
  worktreesDir: '~/.cornercase/worktrees',
  agent: 'claude',
  submit: false,
  trust: true,
  dim: true,
  updates: true,
  agentArgs: { claude: ['--permission-mode', 'plan'] },
  sources: ['all', 'github', 'shortcut', 'linear'],
  accounts: { shortcut: false, linear: false },
});

export const projectLabel = (p: Project) => p.name || p.folder;
export const workspaceLabel = (w: Workspace) => w.name || w.branch || 'default';

export function activePane(t: Tab): Pane | undefined {
  return t.panes.find((p) => p.id === t.active) ?? t.panes[0];
}

export const tabLabel = (t: Tab) => t.name || activePane(t)?.shell.name || 'bash';
