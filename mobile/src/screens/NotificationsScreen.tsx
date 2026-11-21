import { FlatList, RefreshControl, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import type { Paginated } from '../api/types/Paginated';
import type { NotificationDto } from '../api/types/NotificationDto';
import { useAuth } from '../auth';
import { fmtTs } from '../format';
import { Button, Empty } from '../components/ui';
import { makeStyles } from '../theme';

export function useUnreadCount() {
  const { data } = useQuery({
    queryKey: ['notifications-unread'],
    queryFn: () => api.get<{ count: number }>('/api/notifications/unread-count'),
    refetchInterval: 30_000,
  });
  return data?.count ?? 0;
}

export function NotificationsScreen() {
  const styles = useStyles();
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const qc = useQueryClient();
  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: ['notifications'] });
    void qc.invalidateQueries({ queryKey: ['notifications-unread'] });
  };

  const q = useInfiniteQuery({
    queryKey: ['notifications'],
    queryFn: ({ pageParam }) =>
      api.get<Paginated<NotificationDto>>(`/api/notifications?page=${pageParam}&per_page=20`),
    initialPageParam: 1,
    getNextPageParam: (last) =>
      last.page * last.per_page < last.total ? last.page + 1 : undefined,
  });
  const markRead = useMutation({
    mutationFn: (id: string) => api.post(`/api/notifications/${id}/read`),
    onSuccess: invalidate,
  });
  const readAll = useMutation({
    mutationFn: () => api.post('/api/notifications/read-all'),
    onSuccess: invalidate,
  });

  const items = q.data?.pages.flatMap((p) => p.items) ?? [];

  return (
    <View style={styles.wrap}>
      <View style={styles.top}>
        <Button
          title="Прочитать все"
          outline
          loading={readAll.isPending}
          onPress={() => readAll.mutate()}
        />
      </View>
      <FlatList
        data={items}
        keyExtractor={(n) => n.id}
        contentContainerStyle={{ padding: 12 }}
        refreshControl={
          <RefreshControl refreshing={q.isRefetching && !q.isFetchingNextPage} onRefresh={() => void q.refetch()} />
        }
        onEndReached={() => q.hasNextPage && !q.isFetchingNextPage && q.fetchNextPage()}
        onEndReachedThreshold={0.4}
        ListEmptyComponent={q.isLoading ? undefined : <Empty text="Уведомлений нет" />}
        renderItem={({ item }) => (
          <TouchableOpacity
            onPress={() => !item.read_at && markRead.mutate(item.id)}
            style={[styles.item, !item.read_at && styles.unread]}
          >
            <View style={{ flex: 1 }}>
              <Text style={[styles.title, !item.read_at && { fontWeight: '700' }]}>
                {item.title}
              </Text>
              {!!item.body && <Text style={styles.body}>{item.body}</Text>}
              <Text style={styles.ts}>{fmtTs(item.created_at, tz)}</Text>
            </View>
            {!item.read_at && <View style={styles.dot} />}
          </TouchableOpacity>
        )}
      />
    </View>
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  top: { padding: 12, paddingBottom: 0 },
  item: {
    flexDirection: 'row',
    backgroundColor: colors.card,
    borderRadius: 12,
    padding: 12,
    marginBottom: 8,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: colors.border,
    alignItems: 'center',
  },
  unread: { borderLeftWidth: 3, borderLeftColor: colors.primary },
  title: { fontSize: 15, color: colors.text },
  body: { fontSize: 13, color: colors.muted, marginTop: 2 },
  ts: { fontSize: 11, color: colors.muted, marginTop: 4 },
  dot: { width: 8, height: 8, borderRadius: 4, backgroundColor: colors.primary, marginLeft: 8 },
}));
