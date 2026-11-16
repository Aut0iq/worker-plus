import { useState } from 'react';
import {
  Alert,
  Modal,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '@/api/client';
import type { Paginated } from '@/api/types/Paginated';
import type { SessionDto } from '@/api/types/SessionDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes, fmtTs } from '@/format';
import { difficultyLabels } from '@/labels';
import { AuthImage } from '@/components/AuthImage';
import { Button, Card, Empty, ErrorText } from '@/components/ui';
import { colors } from '@/theme';

function useSessions(status: string) {
  return useQuery({
    queryKey: ['kom-sessions', status],
    queryFn: () =>
      api.get<Paginated<SessionDto>>(`/api/komendant/sessions?status=${status}&per_page=50`),
    refetchInterval: 15_000,
  });
}

export default function KomendantSessions() {
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const qc = useQueryClient();
  const pending = useSessions('pending_confirmation');
  const active = useSessions('active');
  const [photoId, setPhotoId] = useState<string | null>(null);
  const [rejectId, setRejectId] = useState<string | null>(null);
  const [reason, setReason] = useState('');
  const [err, setErr] = useState<string | null>(null);

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: ['kom-sessions'] });
  };
  const onErr = (e: unknown) => setErr(e instanceof ApiError ? e.message : 'Ошибка');

  const confirm = useMutation({
    mutationFn: (id: string) => api.post(`/api/komendant/sessions/${id}/confirm`),
    onSuccess: invalidate,
    onError: onErr,
  });
  const reject = useMutation({
    mutationFn: (v: { id: string; reason: string }) =>
      api.post(`/api/komendant/sessions/${v.id}/reject`, { reason: v.reason }),
    onSuccess: () => {
      setRejectId(null);
      setReason('');
      invalidate();
    },
    onError: onErr,
  });

  const pItems = pending.data?.items ?? [];
  const aItems = active.data?.items ?? [];

  const [refreshing, setRefreshing] = useState(false);
  const onRefresh = async () => {
    setRefreshing(true);
    await Promise.all([pending.refetch(), active.refetch()]);
    setRefreshing(false);
  };

  return (
    <ScrollView
      style={styles.wrap}
      contentContainerStyle={{ padding: 12 }}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={() => void onRefresh()} />}
    >
      <ErrorText>{err}</ErrorText>
      <Text style={styles.section}>Ждут подтверждения</Text>
      {pItems.length === 0 && <Empty text="Никого нет" />}
      {pItems.map((s) => (
        <Card key={s.id}>
          <View style={{ flexDirection: 'row' }}>
            {s.has_photo && (
              <Pressable onPress={() => setPhotoId(s.id)}>
                <AuthImage
                  path={`/api/komendant/sessions/${s.id}/photo`}
                  style={styles.thumb}
                  resizeMode="cover"
                />
              </Pressable>
            )}
            <View style={{ flex: 1, marginLeft: s.has_photo ? 10 : 0 }}>
              <Text style={styles.title}>
                {s.student_name ?? '—'}
                {s.group_name ? ` · ${s.group_name}` : ''}
              </Text>
              <Text style={styles.sub}>
                {fmtTs(s.started_at, tz)} — {fmtTs(s.ended_at, tz)}
              </Text>
              <Text style={styles.sub}>
                {fmtClock(s.raw_seconds ?? 0)} → {fmtMinutes(s.rounded_minutes)} × {s.coefficient} ={' '}
                <Text style={{ fontWeight: '700' }}>{fmtMinutes(s.credited_minutes)}</Text>
              </Text>
              <View style={{ flexDirection: 'row', marginTop: 8, gap: 8 }}>
                <Button
                  title="Подтвердить"
                  style={{ flex: 1 }}
                  loading={confirm.isPending && confirm.variables === s.id}
                  onPress={() =>
                    Alert.alert('Подтвердить?', `${s.student_name}: ${fmtMinutes(s.credited_minutes)}`, [
                      { text: 'Отмена', style: 'cancel' },
                      { text: 'Подтвердить', onPress: () => confirm.mutate(s.id) },
                    ])
                  }
                />
                <Button
                  title="Отклонить"
                  danger
                  style={{ flex: 1 }}
                  onPress={() => {
                    setReason('');
                    setRejectId(s.id);
                  }}
                />
              </View>
            </View>
          </View>
        </Card>
      ))}

      <Text style={styles.section}>Сейчас работают</Text>
      {aItems.length === 0 && <Empty text="Никто не работает" />}
      {aItems.map((s) => (
        <Card key={s.id}>
          <Text style={styles.title}>{s.student_name ?? '—'}</Text>
          <Text style={styles.sub}>
            с {fmtTs(s.started_at, tz)} · {difficultyLabels[s.difficulty] ?? s.difficulty}
          </Text>
        </Card>
      ))}

      <Modal visible={!!photoId} onRequestClose={() => setPhotoId(null)}>
        <View style={{ flex: 1, backgroundColor: '#000' }}>
          {photoId && (
            <AuthImage
              path={`/api/komendant/sessions/${photoId}/photo`}
              style={{ flex: 1 }}
              resizeMode="contain"
            />
          )}
          <Button title="Закрыть" onPress={() => setPhotoId(null)} style={styles.modalBtn} />
        </View>
      </Modal>

      <Modal visible={!!rejectId} transparent animationType="fade">
        <View style={styles.modalBg}>
          <View style={styles.modalBox}>
            <Text style={styles.title}>Причина отклонения</Text>
            <TextInput
              style={styles.input}
              value={reason}
              onChangeText={setReason}
              placeholder="Обязательно укажите причину"
              placeholderTextColor={colors.muted}
              multiline
            />
            <View style={{ flexDirection: 'row', gap: 8, marginTop: 12 }}>
              <Button title="Отмена" outline style={{ flex: 1 }} onPress={() => setRejectId(null)} />
              <Button
                title="Отклонить"
                danger
                style={{ flex: 1 }}
                disabled={!reason.trim()}
                loading={reject.isPending}
                onPress={() => rejectId && reject.mutate({ id: rejectId, reason: reason.trim() })}
              />
            </View>
          </View>
        </View>
      </Modal>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  section: { fontSize: 16, fontWeight: '700', color: colors.text, marginBottom: 10, marginTop: 6 },
  title: { fontSize: 15, fontWeight: '600', color: colors.text },
  sub: { fontSize: 12, color: colors.muted, marginTop: 3 },
  thumb: { width: 72, height: 72, borderRadius: 8, backgroundColor: colors.border },
  modalBtn: { position: 'absolute', bottom: 40, left: 24, right: 24 },
  modalBg: {
    flex: 1,
    backgroundColor: '#0008',
    justifyContent: 'center',
    padding: 24,
  },
  modalBox: { backgroundColor: colors.card, borderRadius: 14, padding: 16 },
  input: {
    borderWidth: 1,
    borderColor: colors.border,
    borderRadius: 10,
    padding: 10,
    marginTop: 10,
    minHeight: 70,
    textAlignVertical: 'top',
    color: colors.text,
  },
});
