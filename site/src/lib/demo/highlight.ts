import { BOLD, DIM, ITALIC, type Style } from '../term/grid';
import { type Line, seg } from './text';

const KEYWORDS: Record<string, Set<string>> = {
  rust: new Set(
    'use pub struct impl fn let mut match mod async await return if else for in self Self enum const static where as crate super move loop while break continue true false dyn ref type trait unsafe'.split(' '),
  ),
  ts: new Set('import from export const let var function return new if else type interface async await class extends true false null undefined'.split(' ')),
};

const STYLES: Record<string, Style> = {
  keyword: { fg: 5 },
  type: { fg: 3 },
  string: { fg: 2 },
  comment: { fg: 8, add: ITALIC },
  number: { fg: 3 },
  macro: { fg: 6 },
  attribute: { fg: 4 },
  punct: { fg: 7 },
  key: { fg: 6 },
  heading: { fg: 5, add: BOLD },
  marker: { fg: 6 },
  code: { fg: 2 },
};

export function language(file: string): string {
  if (file.endsWith('.rs')) return 'rust';
  if (/\.(ts|js|mjs|tsx)$/.test(file)) return 'ts';
  if (file.endsWith('.toml') || file.endsWith('.tf')) return 'toml';
  if (file.endsWith('.md')) return 'md';
  return 'text';
}

const TOKEN = /(\/\/.*$|#\[[^\]]*\]|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`[^`]*`|\b\d[\d_.]*\b|\b[A-Za-z_][A-Za-z0-9_]*!?|\s+|.)/g;

function code(text: string, lang: string): Line {
  const out: Line = [];
  const words = KEYWORDS[lang] ?? new Set<string>();
  for (const [token] of text.matchAll(TOKEN)) {
    let style: Style | undefined;
    if (token.startsWith('//')) style = STYLES.comment;
    else if (token.startsWith('#[')) style = STYLES.attribute;
    else if (token.startsWith('"') || token.startsWith('`') || (lang === 'ts' && token.startsWith("'"))) style = STYLES.string;
    else if (/^\d/.test(token)) style = STYLES.number;
    else if (token.endsWith('!') && lang === 'rust') style = STYLES.macro;
    else if (words.has(token)) style = STYLES.keyword;
    else if (/^[A-Z][A-Za-z0-9]*$/.test(token)) style = STYLES.type;
    out.push(seg(token, style));
  }
  return out;
}

function toml(text: string): Line {
  const section = text.match(/^(\s*)(\[.*\])\s*$/);
  if (section) return [seg(section[1]), seg(section[2], { fg: 4, add: BOLD })];
  const pair = text.match(/^(\s*[\w.-]+)(\s*=\s*)(.*)$/);
  if (pair) return [seg(pair[1], STYLES.key), seg(pair[2], STYLES.punct), ...code(pair[3], 'ts')];
  if (text.trim().startsWith('#')) return [seg(text, STYLES.comment)];
  return [seg(text)];
}

function markdown(text: string): Line {
  if (/^#{1,6} /.test(text)) return [seg(text, STYLES.heading)];
  const list = text.match(/^(\s*(?:[-*]|\d+\.)\s)(.*)$/);
  if (list) return [seg(list[1], STYLES.marker), ...inline(list[2])];
  return inline(text);
}

function inline(text: string): Line {
  const out: Line = [];
  for (const [token] of text.matchAll(/(`[^`]+`|\*\*[^*]+\*\*|[^`*]+|.)/g)) {
    if (token.startsWith('`')) out.push(seg(token, STYLES.code));
    else if (token.startsWith('**')) out.push(seg(token, { add: BOLD }));
    else out.push(seg(token));
  }
  return out;
}

export function highlight(text: string, lang: string): Line {
  if (lang === 'rust' || lang === 'ts') return code(text, lang);
  if (lang === 'toml') return toml(text);
  if (lang === 'md') return markdown(text);
  return [seg(text)];
}

export const dim: Style = { fg: 8 };
export const faint: Style = { add: DIM };
