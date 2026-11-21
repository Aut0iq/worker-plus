import { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { router } from 'expo-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, errorText } from '@/api/client';
import type { ActiveSessionResponse } from '@/api/types/ActiveSessionResponse';
import type { SessionDto } from '@/api/types/SessionDto';
import type { UserDto } from '@/api/types/UserDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes } from '@/format';
import { difficultyLabels } from '@/labels';
import { Scanner } from '@/components/Scanner';
import { Button, Card, ErrorText } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

export default function StudentHome() {
  const colors = useColors();
  const styles = useStyles();
  const { config } = useAuth();
  const qc = useQueryClient();
  const [scanning, setScanning] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const me = useQuery({
    queryKey: ['me'],
    queryFn: () => api.get<UserDto>('/api/me'),
    refetchInterval: 30_000,
  });
  const active = useQuery({
    queryKey: ['active-session'],
    queryFn: () => api.get<ActiveSessionResponse>('/api/student/sessions/active'),
    refetchInterval: 15_000,
  });

  const [refreshing, setRefreshing] = useState(false);
  const onRefresh = async () => {
    setRefreshing(true);
    await Promise.all([me.refetch(), active.refetch()]);
    setRefreshing(false);
  };

  // elapsed seconds, tagged with the session id so a stale value is never shown;
  // recomputed every 500ms against the server clock
  const [elapsed, setElapsed] = useState<{ id: string; sec: number } | null>(null);
  const s = active.data?.session ?? null;
  useEffect(() => {
    const serverNow = active.data?.server_now;
    const cur = s?.status === 'active' ? s : null;
    if (!cur || !serverNow) return;
    const offsetMs = Date.parse(serverNow) - Date.now();
    const started = Date.parse(cur.started_at);
    const tick = () =>
      setElapsed({
        id: cur.id,
        sec: Math.max(0, Math.floor((Date.now() + offsetMs - started) / 1000)),
      });
    const t0 = setTimeout(tick, 0);
    const t = setInterval(tick, 500);
    return () => {
      clearTimeout(t0);
      clearInterval(t);
    };
  }, [s, active.data?.server_now]);
  const elapsedSec = s && elapsed?.id === s.id ? elapsed.sec : 0;

  const start = useMutation({
    mutationFn: (qr: string) => api.post<SessionDto>('/api/student/sessions/start', { qr }),
    onSuccess: () => {
      setScanning(false);
      setErr(null);
      void qc.invalidateQueries({ queryKey: ['active-session'] });
    },
    onError: (e) => {
      setScanning(false);
      setErr(errorText(e));
    },
  });

  const u = me.data;
  const maxHours = config?.max_session_hours ?? 4;
  const leftSec = maxHours * 3600 - elapsedSec;
  const overLimit = leftSec <= 0;
  const almostOver = !overLimit && leftSec < 15 * 60;

  return (
    <ScrollView
      style={styles.wrap}
      contentContainerStyle={{ padding: 12 }}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={() => void onRefresh()} />}
    >
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

      {!s && active.isPending && (
        <Card>
          <ActivityIndicator color={colors.primary} />
        </Card>
      )}

      {!s && !active.isPending && (
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
          <Button title="Завершить (сфотографировать работу)" onPress={() => router.push('/finish')} />
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

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  money: { flexDirection: 'row' },
  moneyLabel: { fontSize: 12, color: colors.muted },
  moneyValue: { fontSize: 26, fontWeight: '700', marginTop: 2 },
  hint: { fontSize: 14, color: colors.muted, marginBottom: 12 },
  section: { fontSize: 16, fontWeight: '600', color: colors.text, marginBottom: 6 },
  timer: { fontSize: 44, fontWeight: '700', color: colors.text, textAlign: 'center', marginVertical: 8 },
  sub: { fontSize: 13, color: colors.muted, textAlign: 'center' },
  notice: { fontSize: 12, color: colors.muted, textAlign: 'center', marginVertical: 12 },
}));
