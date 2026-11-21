import { useEffect, type ReactNode } from 'react';
import { AppState, Platform, type AppStateStatus } from 'react-native';
import { DarkTheme, DefaultTheme, Stack, ThemeProvider } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { focusManager, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/auth';
import { ApiError } from '@/api/client';
import { ThemeModeProvider, useColors } from '@/theme';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (count, e) =>
        !(e instanceof ApiError && e.status >= 400 && e.status < 500) && count < 1,
      staleTime: 10_000,
    },
  },
});

/** Headers, tab bars and screen backgrounds follow the app palette (light/dark). */
function NavTheme({ children }: { children: ReactNode }) {
  const c = useColors();
  const base = c.dark ? DarkTheme : DefaultTheme;
  const theme = {
    ...base,
    colors: {
      ...base.colors,
      primary: c.primary,
      background: c.bg,
      card: c.card,
      text: c.text,
      border: c.border,
    },
  };
  return (
    <ThemeProvider value={theme}>
      {children}
      <StatusBar style={c.dark ? 'light' : 'dark'} />
    </ThemeProvider>
  );
}

export default function RootLayout() {
  useEffect(() => {
    const sub = AppState.addEventListener('change', (s: AppStateStatus) => {
      if (Platform.OS !== 'web') focusManager.setFocused(s === 'active');
    });
    return () => sub.remove();
  }, []);

  return (
    <ThemeModeProvider>
      <NavTheme>
        <QueryClientProvider client={queryClient}>
          <AuthProvider>
            <Stack>
              <Stack.Screen name="login" options={{ headerShown: false }} />
              <Stack.Screen name="finish" options={{ title: 'Завершить отработку' }} />
              <Stack.Screen name="qr" options={{ headerShown: false }} />
              <Stack.Screen name="group/[id]" options={{ title: 'Группа' }} />
              <Stack.Screen name="student/[id]" options={{ title: 'Студент' }} />
              <Stack.Screen name="(student)" options={{ headerShown: false }} />
              <Stack.Screen name="(komendant)" options={{ headerShown: false }} />
              <Stack.Screen name="(teacher)" options={{ headerShown: false }} />
            </Stack>
          </AuthProvider>
        </QueryClientProvider>
      </NavTheme>
    </ThemeModeProvider>
  );
}
