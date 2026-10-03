import { type Rect, rect } from '../term/grid';
import { App } from './app';
import { ISSUES } from './data';
import { formArea, issuesArea, layout } from './layout';
import type { IssuesOverlay } from './model';
import { world } from './world';

export interface Scene {
  cols: number;
  rows: number;
  light?: boolean;
  build: (app: App) => Rect | void;
}

function issues(app: App): IssuesOverlay {
  world(app);
  app.openIssues();
  app.advance(600);
  return app.overlay as IssuesOverlay;
}

export const SCENES: Record<string, Scene> = {
  hero: {
    cols: 144,
    rows: 34,
    build: (app) => {
      world(app);
      app.advance(5200);
    },
  },
  overview: {
    cols: 132,
    rows: 30,
    build: (app) => {
      world(app);
      app.advance(9000);
    },
  },
  worktree: {
    cols: 50,
    rows: 14,
    build: (app) => {
      world(app);
      app.advance(4000);
      app.openNewWorkspace();
      for (const ch of 'feat/login') app.key({ key: ch });
      return formArea(app.cols, app.rows);
    },
  },
  issues: {
    cols: 64,
    rows: 17,
    build: (app) => {
      app.config.sources = ['all', 'github', 'shortcut'];
      issues(app);
      return issuesArea(app.cols, app.rows);
    },
  },
  detail: {
    cols: 100,
    rows: 28,
    build: (app) => {
      const o = issues(app);
      o.detail = ISSUES[0].key;
    },
  },
  started: {
    cols: 112,
    rows: 26,
    build: (app) => {
      const o = issues(app);
      app.startIssue(ISSUES[0], o);
      app.advance(7000);
    },
  },
  splits: {
    cols: 112,
    rows: 22,
    build: (app) => {
      world(app);
      const tab = app.tab();
      const area = layout(app.cols, app.rows, app.widths, null).pane;
      const first = tab?.panes[0];
      if (!tab || !first) return;
      app.openPaneMenu({ x: area.x + 2, y: 2 }, tab, first, area);
      app.chooseMenu(0);
      const second = tab.panes[tab.panes.length - 1];
      second.shell.run('cargo test');
      app.advance(6000);
      const at = app.panesOf(tab, area).find(([id]) => id === second.id)?.[1];
      if (at) app.openPaneMenu({ x: at.x + 2, y: at.y + 9 }, tab, second, at);
      return rect(area.x - 1, 0, area.w + 1, app.rows);
    },
  },
  search: {
    cols: 104,
    rows: 11,
    build: (app) => {
      world(app);
      app.openSearch();
      for (const ch of 'feat') app.key({ key: ch });
      return rect(0, 0, 58, app.rows);
    },
  },
  compact: {
    cols: 44,
    rows: 30,
    build: (app) => {
      world(app);
      app.advance(5000);
      app.nav = 'workspaces';
    },
  },
  phone: {
    cols: 44,
    rows: 30,
    build: (app) => {
      world(app);
      app.advance(9000);
    },
  },
  editor: {
    cols: 104,
    rows: 20,
    build: (app) => {
      world(app);
      const p = app.project();
      if (p) {
        p.active = 0;
        p.workspaces[0].active = 0;
      }
      const area = layout(app.cols, app.rows, app.widths, null).pane;
      return rect(area.x, 0, area.w, app.rows);
    },
  },
  copied: {
    cols: 104,
    rows: 16,
    build: (app) => {
      world(app);
      app.advance(2500);
      const p = app.project();
      if (p) {
        p.active = 0;
        p.workspaces[0].active = 1;
      }
      app.notify('copied to clipboard');
      const area = layout(app.cols, app.rows, app.widths, null).pane;
      return rect(area.x, 0, area.w, app.rows);
    },
  },
  sidebar: {
    cols: 104,
    rows: 22,
    build: (app) => {
      world(app);
      app.advance(3000);
      return rect(0, 0, 58, app.rows);
    },
  },
  projects: {
    cols: 104,
    rows: 18,
    build: (app) => {
      world(app);
      return rect(0, 0, 32, app.rows);
    },
  },
  projectsLight: {
    cols: 104,
    rows: 18,
    light: true,
    build: (app) => {
      world(app);
      return rect(0, 0, 32, app.rows);
    },
  },
};

export function run(name: string): { app: App; crop: Rect | null } {
  const scene = SCENES[name];
  if (!scene) throw new Error(`unknown scene: ${name}`);
  const app = new App();
  app.virtual = true;
  app.light = !!scene.light;
  app.resize(scene.cols, scene.rows);
  const crop = scene.build(app) ?? null;
  return { app, crop };
}
