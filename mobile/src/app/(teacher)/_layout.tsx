import { ActivityIndicator, Text, View } from 'react-native';
import { Redirect, Tabs } from 'expo-router';
import { useAuth } from '@/auth';
import { useUnreadCount } from '@/screens/NotificationsScreen';
import { colors } from '@/theme';

const icon = (e: string) =>
  function TabIcon() {
    return <Text style={{ fontSize: 20 }}>{e}</Text>;
  };

export default function TeacherLayout() {
  const { user, ready } = useAuth();
  const unread = useUnreadCount();
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
      <Tabs.Screen name="index" options={{ title: 'Группы', tabBarIcon: icon('👥') }} />
      <Tabs.Screen
        name="notifications"
        options={{
          title: 'Уведомления',
          tabBarIcon: icon('🔔'),
          tabBarBadge: unread > 0 ? unread : undefined,
        }}
      />
      <Tabs.Screen name="profile" options={{ title: 'Профиль', tabBarIcon: icon('👤') }} />
    </Tabs>
  );
}
