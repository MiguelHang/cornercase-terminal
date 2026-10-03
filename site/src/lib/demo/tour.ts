import type { App } from './app';
import type { Mounted } from './client';

export type Step =
  | { say: string }
  | { mark: number }
  | { click: string; nth?: number; dx?: number; right?: boolean; orKey?: string }
  | { until: (app: App) => boolean; timeout: number }
  | { point: string; nth?: number; dx?: number }
  | { key: string }
  | { type: string }
  | { wait: number }
  | { run: (app: App) => void };

export interface Player {
  stop(): void;
  done: Promise<void>;
}

const sleep = (ms: number, signal: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const t = setTimeout(resolve, ms);
    signal.addEventListener('abort', () => {
      clearTimeout(t);
      reject(new Error('stopped'));
    });
  });

function find(app: App, text: string, nth = 0, dx = 0): { x: number; y: number } | null {
  if (app.stale) app.render();
  let p: { x: number; y: number } | null = null;
  if (nth < 0) {
    for (let i = 0; ; i++) {
      const q = app.textAt(text, i);
      if (!q) break;
      p = q;
    }
  } else p = app.textAt(text, nth);
  return p ? { x: p.x + dx, y: p.y } : null;
}

export function play(
  m: Mounted,
  pointer: HTMLElement | null,
  steps: Step[],
  hooks: { say?: (text: string) => void; mark?: (i: number) => void; speed?: number } = {},
): Player {
  const control = new AbortController();
  const { signal } = control;
  const speed = hooks.speed ?? 1;
  const app = m.app;
  const move = async (x: number, y: number) => {
    const at = m.pointTo(x, y);
    if (pointer) {
      pointer.hidden = false;
      pointer.style.transform = `translate(${at.left}px, ${at.top}px)`;
    }
    app.hover = { x, y };
    app.dirty();
    await sleep(620 / speed, signal);
  };
  const tap = (right: boolean) => {
    if (!pointer) return;
    pointer.classList.remove('tap');
    void pointer.offsetWidth;
    pointer.classList.add('tap');
    pointer.classList.toggle('right', right);
  };
  const run = async () => {
    for (const step of steps) {
      if (signal.aborted) return;
      if ('say' in step) hooks.say?.(step.say);
      else if ('mark' in step) hooks.mark?.(step.mark);
      else if ('wait' in step) await sleep(step.wait / speed, signal);
      else if ('run' in step) step.run(app);
      else if ('key' in step) {
        app.key({ key: step.key });
        await sleep(160 / speed, signal);
      } else if ('type' in step) {
        for (const ch of step.type) {
          app.key({ key: ch });
          await sleep((40 + Math.random() * 50) / speed, signal);
        }
      } else if ('point' in step) {
        const p = find(app, step.point, step.nth, step.dx);
        if (p) await move(p.x, p.y);
      } else if ('until' in step) {
        const end = Date.now() + step.timeout / speed;
        while (!step.until(app) && Date.now() < end) await sleep(120, signal);
      } else {
        const p = find(app, step.click, step.nth, step.dx);
        if (!p) {
          if (step.orKey) {
            app.key({ key: step.orKey });
            await sleep(260 / speed, signal);
          }
          continue;
        }
        await move(p.x, p.y);
        tap(!!step.right);
        app.pointerDown(p.x, p.y, step.right ? 2 : 0);
        app.pointerUp();
        await sleep(260 / speed, signal);
      }
    }
  };
  const done = run()
    .catch(() => {})
    .finally(() => {
      if (pointer) pointer.hidden = true;
    });
  return {
    stop() {
      control.abort();
      if (pointer) pointer.hidden = true;
    },
    done,
  };
}
