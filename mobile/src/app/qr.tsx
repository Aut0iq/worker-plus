import { useEffect } from 'react';
import { StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useKeepAwake } from 'expo-keep-awake';
import QRCode from 'react-native-qrcode-svg';
import { api } from '@/api/client';
import type { QrOpenResponse } from '@/api/types/QrOpenResponse';
import { difficultyLabels } from '@/labels';
import { Button } from '@/components/ui';
import { colors } from '@/theme';

export default function QrScreen() {
  useKeepAwake();
  const { difficulty } = useLocalSearchParams<{ difficulty: string }>();
  const d = difficulty === 'hard' ? 'hard' : 'light';
  const { width } = useWindowDimensions();

  const qc = useQueryClient();
  // opening a QR is logged on the server; afterwards we just refresh it
  const open = useMutation({
    mutationFn: () => api.post<QrOpenResponse>('/api/komendant/qr/open', { difficulty: d }),
    onSuccess: (data) => qc.setQueryData(['komendant-qr', d], data),
  });
  useEffect(() => {
    open.mutate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [d]);

  const qr = useQuery({
    queryKey: ['komendant-qr', d],
    queryFn: () => api.get<QrOpenResponse>(`/api/komendant/qr?difficulty=${d}`),
    refetchInterval: 10_000,
  });

  const value = qr.data?.qr;

  return (
    <View style={styles.wrap}>
      <Text style={styles.title}>{difficultyLabels[d]}</Text>
      <View style={styles.qrBox}>
        {value ? (
          <QRCode value={value} size={Math.min(width - 80, 340)} backgroundColor="white" />
        ) : (
          <Text style={styles.sub}>Загрузка…</Text>
        )}
      </View>
      <Text style={styles.sub}>Покажите экран студенту. Код обновляется автоматически.</Text>
      <Button title="Закрыть" outline onPress={() => router.back()} style={styles.close} />
    </View>
  );
}

const styles = StyleSheet.create({
  wrap: {
    flex: 1,
    backgroundColor: colors.card,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
  },
  title: { fontSize: 24, fontWeight: '800', color: colors.text, marginBottom: 24 },
  qrBox: {
    backgroundColor: '#fff',
    padding: 20,
    borderRadius: 16,
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: 200,
    minWidth: 200,
  },
  sub: { fontSize: 13, color: colors.muted, textAlign: 'center', marginTop: 20 },
  close: { marginTop: 24, alignSelf: 'stretch' },
});
