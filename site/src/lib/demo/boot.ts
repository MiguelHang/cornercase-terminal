import type { App } from './app';
import { type Mounted, mount, richText } from './client';
import { activePane } from './model';
import { Agent } from './programs';
import { type Player, type Step, play } from './tour';
import { quiet, world } from './world';

const open = (nav: 'projects' | 'workspaces'): Step => ({
  run: (app) => {
    if (app.cols < 90) {
      app.nav = nav;
      app.dirty();
    }
  },
});

const menu = open('workspaces');

const prompted = (app: App): boolean => {
  const tab = app.tab();
  const agent = tab ? activePane(tab)?.shell.fg : null;
  return agent instanceof Agent && agent.pending.includes('https://');
};

const waitForPrompt: Step = { until: prompted, timeout: 12000 };

export const HERO_TOUR: Step[] = [
  { say: 'Four projects on the left. Each keeps its own workspaces and tabs.' },
  open('projects'),
  { wait: 400 },
  { click: 'api (', nth: 0 },
  { wait: 1500 },
  { say: 'A workspace is a line of work — in a repository, its own git worktree.' },
  open('projects'),
  { wait: 400 },
  { click: 'shop (', nth: 0 },
  { wait: 700 },
  { say: 'Let’s hand an issue to an agent.' },
  menu,
  { click: 'issues', nth: -1 },
  { wait: 1000 },
  { click: '#482' },
  { wait: 1700 },
  { click: 'start   agent', dx: 1, orKey: 'Enter' },
  waitForPrompt,
  { wait: 900 },
  { say: 'The issue is in the prompt. Press Enter and the agent gets to work, in its own worktree.' },
  { key: 'Enter' },
  { wait: 3600 },
  { say: 'Need another terminal next to it? Right-click a pane.' },
  { click: '> https://github.com', right: true },
  { wait: 700 },
  { click: 'split down' },
  { wait: 500 },
  { type: 'git status' },
  { key: 'Enter' },
  { wait: 2600 },
  { say: '`quit` only detaches. The server keeps every shell — and the agent — running.' },
  open('projects'),
  { wait: 400 },
  { click: 'quit', nth: -1 },
  { wait: 4400 },
  { say: 'Your turn: everything here is clickable. Right-click a pane, open settings, search, type in a shell.' },
];

export const FLOW_TOUR: Step[] = [
  { mark: 0 },
  { wait: 900 },
  menu,
  { click: 'issues', nth: -1 },
  { wait: 1100 },
  { point: '#479' },
  { click: '#482' },
  { wait: 2200 },
  { mark: 1 },
  { click: 'start   agent', dx: 1, orKey: 'Enter' },
  { wait: 1400 },
  { mark: 2 },
  { wait: 2000 },
  { mark: 3 },
  waitForPrompt,
  { wait: 900 },
  { key: 'Enter' },
  { mark: 4 },
  { wait: 7200 },
];

interface Config {
  root: HTMLElement;
  pointer: HTMLElement | null;
  narration: HTMLElement | null;
}

function parts(root: HTMLElement): Config {
  return {
    root,
    pointer: root.querySelector<HTMLElement>('.pointer'),
    narration: root.querySelector<HTMLElement>('[data-narration]'),
  };
}

async function hero(root: HTMLElement): Promise<void> {
  const { pointer, narration } = parts(root);
  const say = (html: string) => {
    if (narration) narration.innerHTML = html;
  };
  const make = () =>
    mount(root, {
      rows: (w) => (w < 560 ? 34 : w < 900 ? 30 : 34),
      cell: (w) => (w < 560 ? 7.4 : w < 900 ? 7.6 : 8.4),
      build: (app) => world(app),
      narrate: say,
    });
  let m: Mounted = await make();
  let player: Player | null = null;
  const tourButton = root.querySelector<HTMLButtonElement>('[data-tour]');
  const stop = () => {
    player?.stop();
    player = null;
    tourButton?.setAttribute('aria-pressed', 'false');
  };
  root.addEventListener('demo-interact', () => {
    if (player) stop();
  });
  tourButton?.addEventListener('click', async () => {
    if (player) return stop();
    m.destroy();
    m = await make();
    tourButton.setAttribute('aria-pressed', 'true');
    const current = m;
    player = play(m, pointer, HERO_TOUR, { say: (t) => say(richText(t)) });
    player.done.then(() => {
      current.app.hover = null;
      current.app.dirty();
      if (player) stop();
    });
  });
  root.querySelector<HTMLButtonElement>('[data-reset]')?.addEventListener('click', async () => {
    stop();
    m.destroy();
    m = await make();
    say(richText('Back to the start. Everything is clickable — try right-clicking a pane.'));
  });
  root.dataset.ready = 'true';
  (window as unknown as { demo?: Mounted }).demo = m;
}

async function flow(root: HTMLElement): Promise<void> {
  const { pointer } = parts(root);
  const section = root.closest('section') ?? document;
  const items = [...section.querySelectorAll<HTMLElement>('[data-step]')];
  const mark = (i: number) => items.forEach((el, k) => el.classList.toggle('active', k === i));
  const make = () =>
    mount(root, {
      rows: (w) => (w < 560 ? 30 : 28),
      cell: (w) => (w < 560 ? 7.2 : 7.4),
      build: (app) => quiet(app),
    });
  let m: Mounted = await make();
  root.dataset.ready = 'true';
  let player: Player | null = null;
  let user = false;
  let visible = false;
  const replay = root.querySelector<HTMLButtonElement>('[data-replay]');
  const loop = async () => {
    while (visible && !user) {
      m.destroy();
      m = await make();
      if (!visible || user) return;
      player = play(m, pointer, FLOW_TOUR, { mark });
      await player.done;
      player = null;
      await new Promise((r) => setTimeout(r, 1800));
    }
  };
  root.addEventListener('demo-interact', () => {
    if (user) return;
    user = true;
    player?.stop();
    mark(-1);
    if (replay) replay.hidden = false;
  });
  replay?.addEventListener('click', () => {
    user = false;
    replay.hidden = true;
    loop();
  });
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduced) {
    if (replay) replay.hidden = false;
    user = true;
    return;
  }
  new IntersectionObserver(
    ([entry]) => {
      const was = visible;
      visible = entry.isIntersecting;
      if (visible && !was && !user) loop();
      if (!visible) {
        player?.stop();
        player = null;
      }
    },
    { threshold: 0.35 },
  ).observe(root);
}

export function boot(): void {
  for (const root of document.querySelectorAll<HTMLElement>('[data-demo]')) {
    if (root.dataset.booted) continue;
    root.dataset.booted = 'true';
    const kind = root.dataset.demo;
    if (kind === 'hero') hero(root);
    else if (kind === 'flow') flow(root);
  }
}
