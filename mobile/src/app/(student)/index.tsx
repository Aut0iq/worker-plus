import { useEffect, useRef, useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '@/api/client';
import type { ActiveSessionResponse } from '@/api/types/ActiveSessionResponse';
import type { SessionDto } from '@/api/types/SessionDto';
import type { UserDto } from '@/api/types/UserDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes } from '@/format';
import { difficultyLabels } from '@/labels';
import { Scanner } from '@/components/Scanner';
import { Button, Card, ErrorText } from '@/components/ui';
import { colors } from '@/theme';

export default function StudentHome() {
  const { config } = useAuth();
  const qc = useQueryClient();
  const [scanning, setScanning] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const me = useQuery({ queryKey: ['me'], queryFn: () => api.get<UserDto>('/api/me') });
  const active = useQuery({
    queryKey: ['active-session'],
    queryFn: () => api.get<ActiveSessionResponse>('/api/student/sessions/active'),
    refetchInterval: 15_000,
  });

  // server clock offset, recomputed on every fetch of /active
  const offset = useRef(0);
  useEffect(() => {
    if (active.data?.server_now) {
      offset.current = Date.parse(active.data.server_now) - Date.now();
    }
  }, [active.data?.server_now]);

  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);

  const start = useMutation({
    mutationFn: (qr: string) => api.post<SessionDto>('/api/student/sessions/start', { qr }),
    onSuccess: () => {
      setScanning(false);
      setErr(null);
      void qc.invalidateQueries({ queryKey: ['active-session'] });
    },
    onError: (e) => {
      setScanning(false);
      setErr(e instanceof ApiError ? e.message : 'Ошибка');
    },
  });

  const u = me.data;
  const s = active.data?.session ?? null;
  const maxHours = config?.max_session_hours ?? 4;
  const elapsedSec = s ? Math.floor((now + offset.current - Date.parse(s.started_at)) / 1000) : 0;
  const leftSec = maxHours * 3600 - elapsedSec;
  const overLimit = leftSec <= 0;
  const almostOver = !overLimit && leftSec < 15 * 60;

  return (
    <ScrollView style={styles.wrap} contentContainerStyle={{ padding: 12 }}>
      <Card>
        <View style={styles.money}>
          <View style={{ flex: 1 }}>
            <Text style={styles.moneyLabel}>Долг</Text>
            <Text style={[styles.moneyValue, { color: colors.danger }]}>
              {fmtMinutes(u?.debt_minutes)}
            </Text>
          </View>
          <View style={{ flex: 1 }}>
            <Text style={styles.moneyLabel}>Баланс</Text>
            <Text style={[styles.moneyValue, { color: colors.success }]}>
              {fmtMinutes(u?.balance_minutes)}
            </Text>
          </View>
        </View>
      </Card>

      {!s && (
        <Card>
          <Text style={styles.hint}>Нет активной отработки</Text>
          <Button title="Начать отработку" onPress={() => setScanning(true)} />
          <ErrorText>{err}</ErrorText>
        </Card>
      )}

      {s?.status === 'active' && (
        <Card>
          <Text style={styles.section}>Идёт отработка</Text>
          <Text
            style={[
              styles.timer,
              (overLimit || almostOver) && { color: colors.danger },
            ]}
          >
            {fmtClock(elapsedSec)}
          </Text>
          <Text style={styles.sub}>
            {difficultyLabels[s.difficulty] ?? s.difficulty} · комендант:{' '}
            {s.komendant_name ?? '—'}
          </Text>
          <Text style={[styles.notice, (overLimit || almostOver) && { color: colors.danger }]}>
            Лимит — {maxHours} ч реального времени. Если превысить, отработка не засчитается.
            {overLimit ? ' Лимит превышен!' : ''}
          </Text>
          <Button title="Завершить" onPress={() => router.push('/finish')} />
        </Card>
      )}

      {s?.status === 'pending_confirmation' && (
        <Card>
          <Text style={styles.section}>Ожидает подтверждения коменданта</Text>
          <Text style={styles.sub}>
            Длительность: {fmtClock(s.raw_seconds ?? 0)} · округлено:{' '}
            {fmtMinutes(s.rounded_minutes)} · будет зачтено:{' '}
            <Text style={{ fontWeight: '700' }}>{fmtMinutes(s.credited_minutes)}</Text>
          </Text>
        </Card>
      )}

      <Scanner
        visible={scanning}
        onClose={() => setScanning(false)}
        onScan={(qr) => start.mutate(qr)}
      />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  money: { flexDirection: 'row' },
  moneyLabel: { fontSize: 12, color: colors.muted },
  moneyValue: { fontSize: 26, fontWeight: '700', marginTop: 2 },
  hint: { fontSize: 14, color: colors.muted, marginBottom: 12 },
  section: { fontSize: 16, fontWeight: '600', color: colors.text, marginBottom: 6 },
  timer: { fontSize: 44, fontWeight: '700', color: colors.text, textAlign: 'center', marginVertical: 8 },
  sub: { fontSize: 13, color: colors.muted, textAlign: 'center' },
  notice: { fontSize: 12, color: colors.muted, textAlign: 'center', marginVertical: 12 },
});
