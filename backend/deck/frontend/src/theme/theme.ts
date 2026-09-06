/** Theme engine (spec §5.5): presets are JSON token files under /themes;
 * this module ships the built-in defaults and applies them as CSS custom
 * properties on <html data-theme>. Switching is instant (<100ms budget). */

export interface ThemeTokens {
  bg: string;
  surface: string;
  text: string;
  muted: string;
  accent: string;
  border: string;
}

export interface ThemePreset {
  id: string;
  name: string;
  dark: boolean;
  tokens: ThemeTokens;
}

export const BUILT_IN: ThemePreset[] = [
  {
    id: "cave-dark",
    name: "Bear Cave Dark",
    dark: true,
    tokens: {
      bg: "#101418",
      surface: "#181e25",
      text: "#e6eaee",
      muted: "#8b96a0",
      accent: "#e8a33d",
      border: "#242c35",
    },
  },
  {
    id: "cave-light",
    name: "Bear Cave Light",
    dark: false,
    tokens: {
      bg: "#f6f7f9",
      surface: "#ffffff",
      text: "#1a2027",
      muted: "#5f6b76",
      accent: "#b06f16",
      border: "#dde3e9",
    },
  },
  {
    id: "cave-contrast",
    name: "High Contrast",
    dark: true,
    tokens: {
      bg: "#000000",
      surface: "#0a0a0a",
      text: "#ffffff",
      muted: "#b3b3b3",
      accent: "#ffd400",
      border: "#404040",
    },
  },
];

const STORAGE_KEY = "cave-deck-theme";

export function applyTheme(preset: ThemePreset): void {
  const root = document.documentElement;
  root.setAttribute("data-theme", preset.id);
  for (const [key, value] of Object.entries(preset.tokens)) {
    root.style.setProperty(`--cd-${key.replace(/_/g, "-")}`, value);
  }
  localStorage.setItem(STORAGE_KEY, preset.id);
}

export function currentTheme(): ThemePreset {
  const id = localStorage.getItem(STORAGE_KEY) ?? "cave-dark";
  return BUILT_IN.find((t) => t.id === id) ?? BUILT_IN[0];
}
