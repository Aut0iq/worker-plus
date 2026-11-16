import { FlatList, RefreshControl, StyleSheet, Text, View } from 'react-native';
import { useInfiniteQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { Paginated } from '@/api/types/Paginated';
import type { SessionDto } from '@/api/types/SessionDto';
import { useAuth } from '@/auth';
import { fmtClock, fmtMinutes, fmtTs } from '@/format';
import { difficultyLabels, sessionStatusLabels } from '@/labels';
import { Empty } from '@/components/ui';
import { colors } from '@/theme';

export default function KomendantHistory() {
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const q = useInfiniteQuery({
    queryKey: ['kom-history'],
    queryFn: ({ pageParam }) =>
      api.get<Paginated<SessionDto>>(`/api/komendant/sessions?page=${pageParam}&per_page=20`),
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
      ListEmptyComponent={q.isLoading ? undefined : <Empty text="История пуста" />}
      renderItem={({ item: s }) => {
        const st = sessionStatusLabels[s.status] ?? { label: s.status, color: colors.muted };
        return (
          <View style={styles.item}>
            <View style={{ flex: 1 }}>
              <Text style={styles.title}>
                {s.student_name ?? '—'} · {difficultyLabels[s.difficulty] ?? s.difficulty}
              </Text>
              <Text style={styles.sub}>
                {fmtTs(s.started_at, tz)} · {fmtClock(s.raw_seconds ?? 0)}
                {s.credited_minutes != null ? ` → ${fmtMinutes(s.credited_minutes)}` : ''}
              </Text>
            </View>
            <Text style={[styles.status, { color: st.color }]}>{st.label}</Text>
          </View>
        );
      }}
    />
  );
}

const styles = StyleSheet.create({
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
});
