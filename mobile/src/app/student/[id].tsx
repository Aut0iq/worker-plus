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
import { makeStyles, useColors } from '@/theme';

const MAX_HOURS = 100;

export default function StudentCard() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const colors = useColors();
  const styles = useStyles();
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const qc = useQueryClient();
  const [hours, setHours] = useState('');
  const [err, setErr] = useState<string | null>(null);

  const q = useQuery({
    queryKey: ['student', id],
    queryFn: () => api.get<StudentProfileDto>(`/api/students/${id}`),
  });

  const assign = useMutation({
    mutationFn: (minutes: number) => api.post(`/api/students/${id}/assignments`, { minutes }),
    onSuccess: () => {
      setHours('');
      setErr(null);
      void qc.invalidateQueries({ queryKey: ['student', id] });
      void qc.invalidateQueries({ queryKey: ['debtors'] });
      void qc.invalidateQueries({ queryKey: ['group-students'] });
      Alert.alert('Готово', 'Часы назначены');
    },
    onError: (e) => setErr(e instanceof ApiError ? e.message : 'Ошибка'),
  });

  // whole hours only; a typo like 100 instead of 10 gets a confirmation first
  const submit = () => {
    const h = Number(hours.trim());
    if (!Number.isInteger(h) || h <= 0) return setErr('Укажите целое число часов больше 0');
    if (h > MAX_HOURS) return setErr(`Не больше ${MAX_HOURS} ч за раз`);
    setErr(null);
    Alert.alert('Назначить часы?', `${q.data?.user.full_name ?? 'Студент'}: ${h} ч`, [
      { text: 'Отмена', style: 'cancel' },
      { text: 'Назначить', onPress: () => assign.mutate(h * 60) },
    ]);
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
        <Field
          label={`Часы (1–${MAX_HOURS})`}
          value={hours}
          onChangeText={(t) => setHours(t.replace(/[^0-9]/g, ''))}
          keyboardType="number-pad"
          placeholder="0"
          maxLength={3}
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
              <Text style={styles.rowTitle}>{a.reason || 'Назначено'}</Text>
              <Text style={styles.sub}>
                {a.assigned_by_name ?? '—'} · {fmtTs(a.created_at, tz)}
              </Text>
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

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
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
}));
