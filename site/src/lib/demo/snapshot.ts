import { type Theme, theme as midnight } from '../term/palette';
import { toSvg } from '../term/svg';
import { run } from './scenes';

export function snapshot(name: string, label: string, t: Theme = midnight): { svg: string; cols: number; rows: number } {
  const { app, crop } = run(name);
  const { grid } = app.render();
  const g = crop ? grid.crop(crop) : grid;
  app.destroy();
  return { svg: toSvg(g, { label, theme: t, className: 'term-svg' }), cols: g.cols, rows: g.rows };
}
