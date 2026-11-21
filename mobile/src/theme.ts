import { createContext, createElement, useContext, useEffect, useState, type ReactNode } from 'react';
import { useColorScheme } from 'react-native';
import * as SecureStore from 'expo-secure-store';

export const lightColors = {
  primary: '#208AEF',
  bg: '#f2f4f7',
  card: '#ffffff',
  text: '#1a1a1a',
  muted: '#6b7280',
  border: '#e5e7eb',
  danger: '#d4380d',
  success: '#389e0d',
  warning: '#d48806',
  dark: false,
};

export type Colors = typeof lightColors;

export const darkColors: Colors = {
  primary: '#4ea3f5',
  bg: '#0f1115',
  card: '#1a1d23',
  text: '#eceef2',
  muted: '#9aa1ad',
  border: '#2c313a',
  danger: '#ff7a59',
  success: '#73d13d',
  warning: '#faad14',
  dark: true,
};

/** 'system' follows the phone setting (default). */
export type ThemeMode = 'system' | 'light' | 'dark';

const MODE_KEY = 'theme_mode';

type ThemeCtx = { colors: Colors; mode: ThemeMode; setMode: (m: ThemeMode) => void };

const Ctx = createContext<ThemeCtx>({ colors: lightColors, mode: 'system', setMode: () => {} });

export function ThemeModeProvider({ children }: { children: ReactNode }) {
  const system = useColorScheme();
  const [mode, setModeState] = useState<ThemeMode>('system');

  useEffect(() => {
    SecureStore.getItemAsync(MODE_KEY)
      .then((m) => {
        if (m === 'light' || m === 'dark' || m === 'system') setModeState(m);
      })
      .catch(() => {});
  }, []);

  const setMode = (m: ThemeMode) => {
    setModeState(m);
    SecureStore.setItemAsync(MODE_KEY, m).catch(() => {});
  };

  const dark = mode === 'dark' || (mode === 'system' && system === 'dark');
  const colors = dark ? darkColors : lightColors;
  return createElement(Ctx.Provider, { value: { colors, mode, setMode } }, children);
}

export const useColors = () => useContext(Ctx).colors;
export const useThemeMode = () => useContext(Ctx);

/**
 * Themed styles: `const useStyles = makeStyles((c) => StyleSheet.create({ wrap: { backgroundColor: c.bg } }))`,
 * then `const styles = useStyles()` inside the component. Built once per palette.
 */
export function makeStyles<T>(build: (c: Colors) => T) {
  const cache = new Map<Colors, T>();
  return function useStyles(): T {
    const c = useColors();
    let s = cache.get(c);
    if (!s) {
      s = build(c);
      cache.set(c, s);
    }
    return s;
  };
}
