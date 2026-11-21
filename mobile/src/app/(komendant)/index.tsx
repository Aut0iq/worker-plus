import { StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { Button, Card } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

export default function KomendantHome() {
  const colors = useColors();
  const styles = useStyles();
  return (
    <View style={styles.wrap}>
      <Card>
        <Text style={styles.title}>Открыть QR для отработки</Text>
        <Text style={styles.sub}>Студент сканирует этот код, чтобы начать отработку. Для завершения QR не нужен — только фото.</Text>
        <Button
          title="Лёгкие работы ×1"
          onPress={() => router.push('/qr?difficulty=light')}
          style={{ marginTop: 16 }}
        />
        <Button
          title="Сложные работы ×2"
          onPress={() => router.push('/qr?difficulty=hard')}
          style={{ marginTop: 12, backgroundColor: colors.warning }}
        />
      </Card>
    </View>
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg, justifyContent: 'center', padding: 16 },
  title: { fontSize: 18, fontWeight: '700', color: colors.text, textAlign: 'center' },
  sub: { fontSize: 13, color: colors.muted, textAlign: 'center', marginTop: 8 },
}));
