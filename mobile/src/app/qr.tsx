import { useEffect } from 'react';
import { StyleSheet, Text, useWindowDimensions, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useKeepAwake } from 'expo-keep-awake';
import QRCode from 'react-native-qrcode-svg';
import { api } from '@/api/client';
import type { FinishQrResponse } from '@/api/types/FinishQrResponse';
import { difficultyLabels } from '@/labels';
import { Button } from '@/components/ui';
import { makeStyles } from '@/theme';

// start QR (light/hard, changes every 30 s) or finish QR (`?kind=finish`, every 3 min)
export default function QrScreen() {
  useKeepAwake();
  const styles = useStyles();
  const { difficulty, kind } = useLocalSearchParams<{ difficulty: string; kind: string }>();
  const finish = kind === 'finish';
  const d = difficulty === 'hard' ? 'hard' : 'light';
  const key = finish ? 'finish' : d;
  const { width } = useWindowDimensions();

  const qc = useQueryClient();
  // opening a QR is logged on the server; afterwards we just refresh it
  const open = useMutation({
    mutationFn: () =>
      finish
        ? api.post<FinishQrResponse>('/api/komendant/qr/finish/open')
        : api.post<FinishQrResponse>('/api/komendant/qr/open', { difficulty: d }),
    onSuccess: (data) => qc.setQueryData(['komendant-qr', key], data),
  });
  useEffect(() => {
    open.mutate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const qr = useQuery({
    queryKey: ['komendant-qr', key],
    queryFn: () =>
      api.get<FinishQrResponse>(finish ? '/api/komendant/qr/finish' : `/api/komendant/qr?difficulty=${d}`),
    refetchInterval: 10_000,
  });

  const value = qr.data?.qr;

  return (
    <View style={styles.wrap}>
      <Text style={styles.title}>{finish ? 'Завершение ОПТ' : difficultyLabels[d]}</Text>
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

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
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
}));
