import { useState } from 'react';
import {
  Modal,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { showAlert } from '@/alert';
import { useInfiniteQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, errorText } from '@/api/client';
import type { Paginated } from '@/api/types/Paginated';
import type { SessionDto } from '@/api/types/SessionDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes, fmtTs } from '@/format';
import { difficultyLabels } from '@/labels';
import { AuthImage } from '@/components/AuthImage';
import { Button, Card, Empty, ErrorText } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

// paged: with only the first 50 shown, older pending sessions could never be confirmed
function useSessions(status: string) {
  const q = useInfiniteQuery({
    queryKey: ['kom-sessions', status],
    queryFn: ({ pageParam }) =>
      api.get<Paginated<SessionDto>>(
        `/api/komendant/sessions?status=${status}&page=${pageParam}&per_page=50`,
      ),
    initialPageParam: 1,
    getNextPageParam: (last) =>
      last.page * last.per_page < last.total ? last.page + 1 : undefined,
    refetchInterval: 15_000,
  });
  const items = q.data?.pages.flatMap((p) => p.items) ?? [];
  const total = q.data?.pages[0]?.total ?? 0;
  return { ...q, items, total };
}

function More({ q }: { q: ReturnType<typeof useSessions> }) {
  if (!q.hasNextPage) return null;
  return (
    <Button
      title={`Показать ещё (показано ${q.items.length} из ${q.total})`}
      outline
      loading={q.isFetchingNextPage}
      onPress={() => void q.fetchNextPage()}
      style={{ marginBottom: 12 }}
    />
  );
}

export default function KomendantSessions() {
  const colors = useColors();
  const styles = useStyles();
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
  const onErr = (e: unknown) => setErr(errorText(e));

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

  const pItems = pending.items;
  const aItems = active.items;

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
      <Text style={styles.section}>
        Ждут подтверждения{pending.total ? ` (${pending.total})` : ''}
      </Text>
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
                    showAlert('Подтвердить?', `${s.student_name}: ${fmtMinutes(s.credited_minutes)}`, [
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

      <More q={pending} />

      <Text style={styles.section}>
        Сейчас работают{active.total ? ` (${active.total})` : ''}
      </Text>
      {aItems.length === 0 && <Empty text="Никто не работает" />}
      {aItems.map((s) => (
        <Card key={s.id}>
          <Text style={styles.title}>{s.student_name ?? '—'}</Text>
          <Text style={styles.sub}>
            с {fmtTs(s.started_at, tz)} · {difficultyLabels[s.difficulty] ?? s.difficulty}
          </Text>
        </Card>
      ))}

      <More q={active} />

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

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
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
}));
