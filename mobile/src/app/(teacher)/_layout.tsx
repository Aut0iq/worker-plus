import { ActivityIndicator, View } from 'react-native';
import { Redirect, Tabs } from 'expo-router';
import { useAuth } from '@/auth';
import { tabIcon } from '@/components/Icon';
import { useUnreadCount } from '@/screens/NotificationsScreen';
import { useColors } from '@/theme';

export default function TeacherLayout() {
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
  if (!user || (user.role !== 'teacher' && user.role !== 'admin')) return <Redirect href="/" />;
  return (
    <Tabs screenOptions={{ tabBarActiveTintColor: colors.primary }}>
      <Tabs.Screen name="index" options={{ title: 'Группы', tabBarIcon: tabIcon('group') }} />
      <Tabs.Screen name="debtors" options={{ title: 'Должники', tabBarIcon: tabIcon('warning') }} />
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
