import { Platform } from 'react-native';
import Constants from 'expo-constants';
import * as Device from 'expo-device';
import * as Notifications from 'expo-notifications';
import { api } from './api/client';

let registeredToken: string | null = null;
export const getRegisteredPushToken = () => registeredToken;

if (Platform.OS !== 'web') {
  Notifications.setNotificationHandler({
    handleNotification: async () => ({
      shouldShowBanner: true,
      shouldShowList: true,
      shouldPlaySound: false,
      shouldSetBadge: false,
    }),
  });
}

/** Registers the Expo push token on the server. Never throws — push must not break the app. */
export async function registerPush() {
  try {
    if (Platform.OS === 'web') return;
    // Expo Go (storeClient) has no remote push since SDK 53
    if (Constants.executionEnvironment === 'storeClient') return;
    const projectId = Constants.expoConfig?.extra?.eas?.projectId as string | undefined;
    if (!projectId || !Device.isDevice) return;

    let { status } = await Notifications.getPermissionsAsync();
    if (status !== 'granted') {
      status = (await Notifications.requestPermissionsAsync()).status;
    }
    if (status !== 'granted') return;

    if (Platform.OS === 'android') {
      await Notifications.setNotificationChannelAsync('default', {
        name: 'default',
        importance: Notifications.AndroidImportance.DEFAULT,
      });
    }
    const t = await Notifications.getExpoPushTokenAsync({ projectId });
    registeredToken = t.data;
    await api.post('/api/me/push-token', { token: t.data });
  } catch (e) {
    console.warn('push registration failed', e);
  }
}
