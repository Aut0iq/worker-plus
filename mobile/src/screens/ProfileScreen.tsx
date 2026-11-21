import { useState } from 'react';
import { Alert, FlatList, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useMutation, useQuery } from '@tanstack/react-query';
import { router } from 'expo-router';
import { api, ApiError } from '../api/client';
import type { Paginated } from '../api/types/Paginated';
import type { AssignmentDto } from '../api/types/AssignmentDto';
import { useAuth } from '../auth';
import { fmtMinutes, fmtTs } from '../format';
import { roleLabels } from '../labels';
import { Button, Card, Empty, ErrorText, Field } from '../components/ui';
import { makeStyles, useColors, useThemeMode, type ThemeMode } from '../theme';

const THEME_OPTIONS: { mode: ThemeMode; label: string }[] = [
  { mode: 'system', label: 'Как в системе' },
  { mode: 'light', label: 'Светлая' },
  { mode: 'dark', label: 'Тёмная' },
];

export function ProfileScreen() {
  const { user, config, logout, updateToken } = useAuth();
  const colors = useColors();
  const styles = useStyles();
  const { mode, setMode } = useThemeMode();
  const tz = config?.timezone ?? 'UTC';
  const [oldPwd, setOldPwd] = useState('');
  const [newPwd, setNewPwd] = useState('');
  const [newPwd2, setNewPwd2] = useState('');
  const [pwdErr, setPwdErr] = useState<string | null>(null);

  const assignments = useQuery({
    queryKey: ['my-assignments'],
    enabled: user?.role === 'student',
    queryFn: () => api.get<Paginated<AssignmentDto>>('/api/student/assignments?page=1&per_page=50'),
  });

  const changePwd = useMutation({
    mutationFn: () =>
      api.post<{ token?: string }>('/api/me/password', { old_password: oldPwd, new_password: newPwd }),
    onSuccess: async (r) => {
      // old tokens are revoked by the server; keep this device logged in
      if (r?.token) await updateToken(r.token);
      setOldPwd('');
      setNewPwd('');
      setNewPwd2('');
      setPwdErr(null);
      Alert.alert('Готово', 'Пароль изменён');
    },
    onError: (e) => setPwdErr(e instanceof ApiError ? e.message : 'Ошибка'),
  });

  const submitPwd = () => {
    if (newPwd.length < 6) return setPwdErr('Новый пароль — минимум 6 символов');
    if (newPwd !== newPwd2) return setPwdErr('Пароли не совпадают');
    changePwd.mutate();
  };

  if (!user) return null;

  return (
    <ScrollView style={styles.wrap} contentContainerStyle={{ padding: 12 }}>
      <Card>
        <Text style={styles.name}>{user.full_name}</Text>
        <Text style={styles.sub}>
          {user.login} · {roleLabels[user.role] ?? user.role}
          {user.group_name ? ` · ${user.group_name}` : ''}
        </Text>
        <View style={styles.money}>
          <View style={{ flex: 1 }}>
            <Text style={styles.moneyLabel}>Долг</Text>
            <Text style={[styles.moneyValue, { color: colors.danger }]}>
              {fmtMinutes(user.debt_minutes)}
            </Text>
          </View>
          <View style={{ flex: 1 }}>
            <Text style={styles.moneyLabel}>Баланс</Text>
            <Text style={[styles.moneyValue, { color: colors.success }]}>
              {fmtMinutes(user.balance_minutes)}
            </Text>
          </View>
        </View>
      </Card>

      {user.role === 'student' && (
        <Card>
          <Text style={styles.section}>Назначенные часы</Text>
          <FlatList
            data={assignments.data?.items ?? []}
            keyExtractor={(a) => a.id}
            scrollEnabled={false}
            ListEmptyComponent={
              assignments.isLoading ? undefined : <Empty text="Назначений нет" />
            }
            renderItem={({ item }) => (
              <View style={styles.assign}>
                <View style={{ flex: 1 }}>
                  <Text style={styles.assignReason}>{item.reason || 'Назначено'}</Text>
                  <Text style={styles.sub}>
                    {item.assigned_by_name ?? '—'} · {fmtTs(item.created_at, tz)}
                  </Text>
                </View>
                <Text style={styles.assignMin}>{fmtMinutes(item.minutes)}</Text>
              </View>
            )}
          />
        </Card>
      )}

      <Card>
        <Text style={styles.section}>Тема оформления</Text>
        <View style={styles.segment}>
          {THEME_OPTIONS.map((o) => (
            <Pressable
              key={o.mode}
              onPress={() => setMode(o.mode)}
              style={[styles.segItem, mode === o.mode && { backgroundColor: colors.primary }]}
            >
              <Text style={[styles.segText, mode === o.mode && { color: '#fff' }]}>{o.label}</Text>
            </Pressable>
          ))}
        </View>
      </Card>

      <Card>
        <Text style={styles.section}>Сменить пароль</Text>
        <Field
          label="Текущий пароль"
          value={oldPwd}
          onChangeText={setOldPwd}
          secureTextEntry
          autoCapitalize="none"
        />
        <Field
          label="Новый пароль (мин. 6 символов)"
          value={newPwd}
          onChangeText={setNewPwd}
          secureTextEntry
          autoCapitalize="none"
        />
        <Field
          label="Новый пароль ещё раз"
          value={newPwd2}
          onChangeText={setNewPwd2}
          secureTextEntry
          autoCapitalize="none"
        />
        <ErrorText>{pwdErr}</ErrorText>
        <Button title="Сменить пароль" loading={changePwd.isPending} onPress={submitPwd} />
      </Card>

      <Button
        title="Выйти"
        danger
        onPress={() => {
          void logout().then(() => router.replace('/login'));
        }}
        style={{ marginTop: 4, marginBottom: 32 }}
      />
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
  assign: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 8,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: colors.border,
  },
  assignReason: { fontSize: 14, color: colors.text },
  segment: {
    flexDirection: 'row',
    borderWidth: 1,
    borderColor: colors.border,
    borderRadius: 10,
    overflow: 'hidden',
  },
  segItem: { flex: 1, paddingVertical: 10, alignItems: 'center' },
  segText: { fontSize: 13, color: colors.text },
  assignMin: { fontSize: 15, fontWeight: '600', color: colors.primary, marginLeft: 8 },
}));
