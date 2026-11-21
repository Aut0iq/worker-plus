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
        <Text style={styles.title}>Открыть QR для ОПТ</Text>
        <Text style={styles.sub}>Студент сканирует этот код, чтобы начать общественно полезный труд.</Text>
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
      <Card>
        <Text style={styles.title}>QR для завершения ОПТ</Text>
        <Text style={styles.sub}>
          Студент фотографирует результат работы, затем сканирует этот код. Код меняется раз в 3 минуты.
        </Text>
        <Button title="Открыть QR завершения" onPress={() => router.push('/qr?kind=finish')} style={{ marginTop: 16 }} />
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
