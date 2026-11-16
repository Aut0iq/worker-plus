import { useEffect } from 'react';
import { AppState, Platform, type AppStateStatus } from 'react-native';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { focusManager, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/auth';
import { ApiError } from '@/api/client';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (count, e) =>
        !(e instanceof ApiError && e.status >= 400 && e.status < 500) && count < 1,
      staleTime: 10_000,
    },
  },
});

export default function RootLayout() {
  useEffect(() => {
    const sub = AppState.addEventListener('change', (s: AppStateStatus) => {
      if (Platform.OS !== 'web') focusManager.setFocused(s === 'active');
    });
    return () => sub.remove();
  }, []);

  return (
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
        <StatusBar style="auto" />
      </AuthProvider>
    </QueryClientProvider>
  );
}
