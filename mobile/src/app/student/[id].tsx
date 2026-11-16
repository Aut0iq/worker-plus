import { useState } from 'react';
import { Alert, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useLocalSearchParams } from 'expo-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '@/api/client';
import type { StudentProfileDto } from '@/api/types/StudentProfileDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes, fmtTs } from '@/format';
import { difficultyLabels, sessionStatusLabels } from '@/labels';
import { Button, Card, Empty, ErrorText, Field } from '@/components/ui';
import { colors } from '@/theme';

export default function StudentCard() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const qc = useQueryClient();
  const [hours, setHours] = useState('');
  const [mins, setMins] = useState('');
  const [reason, setReason] = useState('');
  const [err, setErr] = useState<string | null>(null);

  const q = useQuery({
    queryKey: ['student', id],
    queryFn: () => api.get<StudentProfileDto>(`/api/students/${id}`),
  });

  const assign = useMutation({
    mutationFn: (v: { minutes: number; reason: string }) =>
      api.post(`/api/students/${id}/assignments`, v),
    onSuccess: () => {
      setHours('');
      setMins('');
      setReason('');
      setErr(null);
      void qc.invalidateQueries({ queryKey: ['student', id] });
      Alert.alert('Готово', 'Часы назначены');
    },
    onError: (e) => setErr(e instanceof ApiError ? e.message : 'Ошибка'),
  });

  const submit = () => {
    const total = (parseInt(hours || '0', 10) || 0) * 60 + (parseInt(mins || '0', 10) || 0);
    if (total <= 0) return setErr('Укажите время больше 0');
    if (!reason.trim()) return setErr('Укажите причину');
    assign.mutate({ minutes: total, reason: reason.trim() });
  };

  const u = q.data?.user;
  const sessions = q.data?.recent_sessions ?? [];
  const assignments = q.data?.assignments ?? [];

  return (
    <ScrollView style={styles.wrap} contentContainerStyle={{ padding: 12 }}>
      {u && (
        <Card>
          <Text style={styles.name}>{u.full_name}</Text>
          <Text style={styles.sub}>{u.group_name ?? '—'}</Text>
          <View style={styles.money}>
            <View style={{ flex: 1 }}>
              <Text style={styles.moneyLabel}>Долг</Text>
              <Text style={[styles.moneyValue, { color: colors.danger }]}>
                {fmtMinutes(u.debt_minutes)}
              </Text>
            </View>
            <View style={{ flex: 1 }}>
              <Text style={styles.moneyLabel}>Баланс</Text>
              <Text style={[styles.moneyValue, { color: colors.success }]}>
                {fmtMinutes(u.balance_minutes)}
              </Text>
            </View>
          </View>
        </Card>
      )}

      <Card>
        <Text style={styles.section}>Назначить часы</Text>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          <View style={{ flex: 1 }}>
            <Field
              label="Часы (0–100)"
              value={hours}
              onChangeText={setHours}
              keyboardType="number-pad"
              placeholder="0"
            />
          </View>
          <View style={{ flex: 1 }}>
            <Field
              label="Минуты (0–59)"
              value={mins}
              onChangeText={setMins}
              keyboardType="number-pad"
              placeholder="0"
            />
          </View>
        </View>
        <Field
          label="Причина (обязательно)"
          value={reason}
          onChangeText={setReason}
          placeholder="За что назначаются часы"
        />
        <ErrorText>{err}</ErrorText>
        <Button title="Назначить" loading={assign.isPending} onPress={submit} />
      </Card>

      <Card>
        <Text style={styles.section}>Последние отработки</Text>
        {sessions.length === 0 && <Empty text="Отработок нет" />}
        {sessions.map((s) => {
          const st = sessionStatusLabels[s.status] ?? { label: s.status, color: colors.muted };
          return (
            <View key={s.id} style={styles.row}>
              <View style={{ flex: 1 }}>
                <Text style={styles.rowTitle}>
                  {fmtTs(s.started_at, tz)} · {difficultyLabels[s.difficulty] ?? s.difficulty}
                </Text>
                <Text style={styles.sub}>
                  {fmtClock(s.raw_seconds ?? 0)}
                  {s.credited_minutes != null ? ` → ${fmtMinutes(s.credited_minutes)}` : ''}
                </Text>
              </View>
              <Text style={{ color: st.color, fontSize: 12, fontWeight: '600' }}>{st.label}</Text>
            </View>
          );
        })}
      </Card>

      <Card>
        <Text style={styles.section}>Назначения</Text>
        {assignments.length === 0 && <Empty text="Назначений нет" />}
        {assignments.map((a) => (
          <View key={a.id} style={styles.row}>
            <View style={{ flex: 1 }}>
              <Text style={styles.rowTitle}>{a.reason}</Text>
              <Text style={styles.sub}>{fmtTs(a.created_at, tz)}</Text>
            </View>
            <Text style={{ color: colors.primary, fontWeight: '600' }}>
              {fmtMinutes(a.minutes)}
            </Text>
          </View>
        ))}
      </Card>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  name: { fontSize: 20, fontWeight: '700', color: colors.text },
  sub: { fontSize: 13, color: colors.muted, marginTop: 2 },
  money: { flexDirection: 'row', marginTop: 12 },
  moneyLabel: { fontSize: 12, color: colors.muted },
  moneyValue: { fontSize: 22, fontWeight: '700', marginTop: 2 },
  section: { fontSize: 16, fontWeight: '600', color: colors.text, marginBottom: 10 },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 8,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: colors.border,
  },
  rowTitle: { fontSize: 14, color: colors.text },
});
