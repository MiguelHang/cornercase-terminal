import { BOLD } from '../term/grid';
import { App } from './app';
import { ADDRESS_RS, COMMITS, FOLDERS } from './data';
import type { Project } from './model';
import { Agent, Editor } from './programs';
import { type Line, seg } from './text';

function prompt(folder: string, command: string): Line {
  return [seg(folder, { fg: 6, add: BOLD }), seg(' '), seg('❯', { fg: 5 }), seg(` ${command}`)];
}

function gitLog(project: string, branch: string): Line[] {
  return (COMMITS[project] ?? []).map(([hash, msg], i) => [
    seg('* ', { fg: 1 }),
    seg(hash, { fg: 3 }),
    ...(i === 0 ? [seg(' ('), seg('HEAD -> ', { fg: 6, add: BOLD }), seg(branch, { fg: 2, add: BOLD }), seg(')', { fg: 3 })] : []),
    seg(` ${msg}`),
  ]);
}

function tests(root: string): Line[] {
  const head = (s: string) => seg(s.padStart(12), { fg: 2, add: BOLD });
  const ok = (t: string): Line => [seg(`test ${t} ... `), seg('ok', { fg: 2 })];
  return [
    prompt('feat-dark-mode', 'cargo test'),
    [head('Compiling'), seg(` shop v0.5.0 (${root})`)],
    [head('Finished'), seg(' `test` profile [unoptimized + debuginfo] target(s) in 2.41s')],
    [head('Running'), seg(' unittests src/main.rs')],
    [],
    [seg('running 7 tests')],
    ok('checkout::tests::totals_include_tax'),
    ok('checkout::tests::coupons_stack'),
    ok('returns::tests::label_is_printed'),
    ok('theme::tests::dark_background_is_dark'),
    ok('theme::tests::light_is_the_default'),
    ok('returns::address::tests::postcode_is_trimmed'),
    [seg('test returns::address::tests::empty_address_is_rejected ... '), seg('FAILED', { fg: 1 })],
    [],
    [seg('test result: '), seg('FAILED', { fg: 1 }), seg('. 6 passed; 1 failed; 0 ignored; finished in 0.02s')],
  ];
}

function inGroup(app: App, name: string, projects: Project[]): void {
  const group = app.addGroup(name, '●', 12);
  for (const p of projects) p.group = group.id;
}

export function world(app: App): App {
  const shop = app.addProject('shop', '~/code/shop', true, FOLDERS.shop.tree);
  const main = shop.workspaces[0];
  const editorPane = main.tabs[0].panes[0];
  editorPane.shell.start(new Editor(app.host(shop, main, () => editorPane.id), () => editorPane.shell.finish(), 'src/returns/address.rs', ADDRESS_RS, 11));
  main.behind = 2;
  const logTab = app.newTab([app.newPane(shop, main, [prompt('shop', 'git log --oneline --graph'), ...gitLog('shop', 'main'), prompt('shop', 'git status -sb'), [seg('## main...origin/main [behind 2]')]])]);
  main.tabs.push(logTab);

  const dark = app.addWorkspace(shop, 'feat/dark-mode', true);
  const agentPane = app.newPane(shop, dark);
  const testsPane = app.newPane(shop, dark, tests(dark.root));
  const shellPane = app.newPane(shop, dark, [prompt('feat-dark-mode', 'git status -sb'), [seg('## feat/dark-mode')], [seg(' M ', { fg: 1 }), seg('src/theme.rs')]]);
  agentPane.shell.start(
    new Agent(app.host(shop, dark, () => agentPane.id), () => agentPane.shell.finish(), 'claude', ['--permission-mode', 'plan'], {
      working: 'https://github.com/acme/shop/issues/479',
    }),
  );
  if (app.cols < 130) {
    dark.tabs.push(app.newTab([agentPane]), app.newTab([testsPane]));
  } else {
    dark.tabs.push(
      app.newTab([agentPane, testsPane, shellPane], {
        dir: 'right',
        ratio: 0.56,
        first: { leaf: agentPane.id },
        second: { dir: 'down', ratio: 0.62, first: { leaf: testsPane.id }, second: { leaf: shellPane.id } },
      }),
    );
  }
  shop.active = 1;

  const api = app.addProject('api', '~/code/api', true, FOLDERS.api.tree);
  const apiMain = api.workspaces[0];
  apiMain.tabs[0].panes[0].shell.run('npm run dev');
  const apiShell = app.newPane(api, apiMain, [prompt('api', 'git log --oneline'), ...gitLog('api', 'main')]);
  apiMain.tabs.push(app.newTab([apiShell]));
  const fix = app.addWorkspace(api, 'fix/pagination', true);
  fix.tabs.push(app.newTab([app.newPane(api, fix)]));

  const infra = app.addProject('infra', '~/code/infra', true, FOLDERS.infra.tree);
  const notes = app.addProject('notes', '~/code/notes', false, FOLDERS.notes.tree);
  const notesPane = notes.workspaces[0].tabs[0].panes[0];
  notesPane.shell.run('cat todo.md');
  inGroup(app, 'acme', [shop, api, infra]);

  app.active = 0;
  return app;
}

export function quiet(app: App): App {
  const shop = app.addProject('shop', '~/code/shop', true, FOLDERS.shop.tree);
  const main = shop.workspaces[0];
  const editorPane = main.tabs[0].panes[0];
  editorPane.shell.start(new Editor(app.host(shop, main, () => editorPane.id), () => editorPane.shell.finish(), 'src/returns/address.rs', ADDRESS_RS, 11));
  main.tabs.push(app.newTab([app.newPane(shop, main, [prompt('shop', 'git log --oneline --graph'), ...gitLog('shop', 'main')])]));
  const dark = app.addWorkspace(shop, 'feat/dark-mode', true);
  dark.tabs.push(app.newTab([app.newPane(shop, dark, [prompt('feat-dark-mode', 'git status -sb'), [seg('## feat/dark-mode')]])]));
  const api = app.addProject('api', '~/code/api', true, FOLDERS.api.tree);
  const infra = app.addProject('infra', '~/code/infra', true, FOLDERS.infra.tree);
  app.addProject('notes', '~/code/notes', false, FOLDERS.notes.tree);
  inGroup(app, 'acme', [shop, api, infra]);
  app.active = 0;
  return app;
}
