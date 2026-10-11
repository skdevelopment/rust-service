/**
 * Bundled portable fonts + CSS-variable application.
 * Family names and stacks must stay in sync with
 * `src-tauri/src/commands/fonts.rs`.
 */

export const FONT_CSS_VAR_SANS = '--app-font-sans';
export const FONT_CSS_VAR_MONO = '--app-font-mono';
export const FONT_CSS_VAR_BODY = '--font-body';
export const FONT_CSS_VAR_HEADING = '--font-heading';

export const DEFAULT_SANS_STACK =
  'ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"';
export const DEFAULT_MONO_STACK =
  'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace';
export const DEFAULT_BODY_STACK =
  '"Segoe UI", system-ui, -apple-system, Roboto, Helvetica, Arial, sans-serif';
export const DEFAULT_HEADING_STACK = '"Inter", "Segoe UI", system-ui, sans-serif';

export interface BundledFont {
  family: string;
  isMonospace: boolean;
}

/** Five current UI fonts + five current code fonts, shipped in public/fonts. */
export const BUNDLED_FONTS: BundledFont[] = [
  { family: 'Inter', isMonospace: false },
  { family: 'Geist', isMonospace: false },
  { family: 'IBM Plex Sans', isMonospace: false },
  { family: 'Plus Jakarta Sans', isMonospace: false },
  { family: 'Manrope', isMonospace: false },
  { family: 'JetBrains Mono', isMonospace: true },
  { family: 'Fira Code', isMonospace: true },
  { family: 'Geist Mono', isMonospace: true },
  { family: 'Source Code Pro', isMonospace: true },
  { family: 'Roboto Mono', isMonospace: true },
];

export const BUNDLED_UI_FONTS = BUNDLED_FONTS.filter((f) => !f.isMonospace);
export const BUNDLED_MONO_FONTS = BUNDLED_FONTS.filter((f) => f.isMonospace);

export interface FontCssBinding {
  sans: string;
  mono: string;
  body: string;
  heading: string;
  sansVariable: string;
  monoVariable: string;
}

function cssQuotedFamily(name: string): string {
  return `"${name.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

export function resolveBundledFamily(name: string, monospace: boolean): string | null {
  const trimmed = name.trim();
  if (!trimmed) {
    return null;
  }
  const match = BUNDLED_FONTS.find(
    (font) => font.isMonospace === monospace && font.family.toLowerCase() === trimmed.toLowerCase(),
  );
  return match?.family ?? null;
}

/** Map persisted UI/mono names to the CSS values the document applies. */
export function fontCssBinding(uiFont: string, monoFont: string): FontCssBinding {
  const ui = resolveBundledFamily(uiFont, false);
  const mono = resolveBundledFamily(monoFont, true);

  const sans = ui ? `${cssQuotedFamily(ui)}, ${DEFAULT_SANS_STACK}` : DEFAULT_SANS_STACK;
  const body = ui ? sans : DEFAULT_BODY_STACK;
  const heading = ui ? sans : DEFAULT_HEADING_STACK;
  const monoStack = mono ? `${cssQuotedFamily(mono)}, ${DEFAULT_MONO_STACK}` : DEFAULT_MONO_STACK;

  return {
    sans,
    mono: monoStack,
    body,
    heading,
    sansVariable: FONT_CSS_VAR_SANS,
    monoVariable: FONT_CSS_VAR_MONO,
  };
}

export function applyFontCssBinding(binding: FontCssBinding): void {
  if (typeof document === 'undefined') {
    return;
  }
  const root = document.documentElement;
  root.style.setProperty(FONT_CSS_VAR_SANS, binding.sans);
  root.style.setProperty(FONT_CSS_VAR_MONO, binding.mono);
  root.style.setProperty('--font-sans', binding.sans);
  root.style.setProperty('--font-mono', binding.mono);
  root.style.setProperty(FONT_CSS_VAR_BODY, binding.body);
  root.style.setProperty(FONT_CSS_VAR_HEADING, binding.heading);
  root.style.fontFamily = binding.sans;
  if (document.body) {
    document.body.style.fontFamily = binding.sans;
  }
}

/** Apply immediately on the document. Unknown names fall back to the default stacks. */
export function applyAppFonts(uiFont: string, monoFont: string): void {
  applyFontCssBinding(fontCssBinding(uiFont ?? '', monoFont ?? ''));
}
