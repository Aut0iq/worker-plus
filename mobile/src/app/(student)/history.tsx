import { FlatList, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useInfiniteQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { Paginated } from '@/api/types/Paginated';
import type { SessionDto } from '@/api/types/SessionDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes, fmtTs } from '@/format';
import { difficultyLabels, photoReviewLabels, sessionStatusLabels } from '@/labels';
import { Empty } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

function Row({ s, tz }: { s: SessionDto; tz: string }) {
  const colors = useColors();
  const styles = useStyles();
  const st = sessionStatusLabels[s.status] ?? { label: s.status, color: colors.muted };
  const pr = s.photo_review ? photoReviewLabels[s.photo_review] : null;
  return (
    <View style={styles.item}>
      <View style={{ flex: 1 }}>
        <Text style={styles.title}>
          {fmtTs(s.started_at, tz)} · {difficultyLabels[s.difficulty] ?? s.difficulty}
        </Text>
        <Text style={styles.sub}>
          {s.komendant_name ?? '—'} · {fmtClock(s.raw_seconds ?? 0)} → {fmtMinutes(s.rounded_minutes)}
          {s.credited_minutes != null ? ` → зачтено ${fmtMinutes(s.credited_minutes)}` : ''}
        </Text>
        {pr && (
          <Text style={[styles.sub, { color: pr.color }]}>Фото: {pr.label}</Text>
        )}
        {!!s.rejected_reason && (
          <Text style={[styles.sub, { color: colors.danger }]}>Причина: {s.rejected_reason}</Text>
        )}
        {!!s.photo_review_comment && (
          <Text style={[styles.sub, { color: colors.danger }]}>
            Комментарий: {s.photo_review_comment}
          </Text>
        )}
      </View>
      <Text style={[styles.status, { color: st.color }]}>{st.label}</Text>
    </View>
  );
}

export default function StudentHistory() {
  const colors = useColors();
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const q = useInfiniteQuery({
    queryKey: ['my-sessions'],
    queryFn: ({ pageParam }) =>
      api.get<Paginated<SessionDto>>(`/api/student/sessions?page=${pageParam}&per_page=20`),
    initialPageParam: 1,
    getNextPageParam: (last) =>
      last.page * last.per_page < last.total ? last.page + 1 : undefined,
  });
  const items = q.data?.pages.flatMap((p) => p.items) ?? [];
  return (
    <FlatList
      style={{ flex: 1, backgroundColor: colors.bg }}
      contentContainerStyle={{ padding: 12 }}
      data={items}
      keyExtractor={(s) => s.id}
      refreshControl={
        <RefreshControl refreshing={q.isRefetching && !q.isFetchingNextPage} onRefresh={() => void q.refetch()} />
      }
      onEndReached={() => q.hasNextPage && !q.isFetchingNextPage && q.fetchNextPage()}
      onEndReachedThreshold={0.4}
      ListEmptyComponent={q.isLoading ? undefined : <Empty text="Записей ОПТ пока нет" />}
      renderItem={({ item }) => <Row s={item} tz={tz} />}
    />
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  item: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: colors.card,
    borderRadius: 12,
    padding: 12,
    marginBottom: 8,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: colors.border,
  },
  title: { fontSize: 14, fontWeight: '600', color: colors.text },
  sub: { fontSize: 12, color: colors.muted, marginTop: 2 },
  status: { fontSize: 12, fontWeight: '600', marginLeft: 8, maxWidth: 110, textAlign: 'right' },
}));
