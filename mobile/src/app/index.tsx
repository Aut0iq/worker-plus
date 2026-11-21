import { ActivityIndicator, View } from 'react-native';
import { Redirect } from 'expo-router';
import { useAuth } from '@/auth';
import { useColors } from '@/theme';

export default function Index() {
  const { user, ready } = useAuth();
  const colors = useColors();
  if (!ready) {
    return (
      <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
        <ActivityIndicator size="large" color={colors.primary} />
      </View>
    );
  }
  if (!user) return <Redirect href="/login" />;
  if (user.role === 'student') return <Redirect href="/(student)" />;
  if (user.role === 'komendant') return <Redirect href="/(komendant)" />;
  return <Redirect href="/(teacher)" />;
}
