import { ActivityIndicator, View } from 'react-native';
import { Redirect, Tabs } from 'expo-router';
import { useAuth } from '@/auth';
import { tabIcon } from '@/components/Icon';
import { useUnreadCount } from '@/screens/NotificationsScreen';
import { useColors } from '@/theme';

export default function StudentLayout() {
  const { user, ready } = useAuth();
  const unread = useUnreadCount();
  const colors = useColors();
  if (!ready) {
    return (
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <ActivityIndicator size="large" color={colors.primary} />
      </View>
    );
  }
  if (!user || user.role !== 'student') return <Redirect href="/" />;
  return (
    <Tabs screenOptions={{ tabBarActiveTintColor: colors.primary }}>
      <Tabs.Screen name="index" options={{ title: 'Главная', tabBarIcon: tabIcon('home') }} />
      <Tabs.Screen name="history" options={{ title: 'История', tabBarIcon: tabIcon('history') }} />
      <Tabs.Screen name="shop" options={{ title: 'Магазин', tabBarIcon: tabIcon('gift') }} />
      <Tabs.Screen
        name="notifications"
        options={{
          title: 'Уведомления',
          tabBarIcon: tabIcon('bell'),
          tabBarBadge: unread > 0 ? unread : undefined,
        }}
      />
      <Tabs.Screen name="profile" options={{ title: 'Профиль', tabBarIcon: tabIcon('person') }} />
    </Tabs>
  );
}
