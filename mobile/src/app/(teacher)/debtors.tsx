import { FlatList, RefreshControl, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { router } from 'expo-router';
import { useInfiniteQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { Paginated } from '@/api/types/Paginated';
import type { UserDto } from '@/api/types/UserDto';
import { fmtMinutes } from '@/format';
import { Icon } from '@/components/Icon';
import { Empty } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

/** Students who still owe hours, largest debt first. */
export default function Debtors() {
  const colors = useColors();
  const styles = useStyles();
  const q = useInfiniteQuery({
    queryKey: ['debtors'],
    queryFn: ({ pageParam }) =>
      api.get<Paginated<UserDto>>(`/api/students/debtors?page=${pageParam}&per_page=50`),
    initialPageParam: 1,
    getNextPageParam: (last) =>
      last.page * last.per_page < last.total ? last.page + 1 : undefined,
  });
  const items = q.data?.pages.flatMap((p) => p.items) ?? [];
  const total = q.data?.pages[0]?.total ?? 0;
  return (
    <FlatList
      style={{ flex: 1, backgroundColor: colors.bg }}
      contentContainerStyle={{ padding: 12 }}
      data={items}
      keyExtractor={(u) => u.id}
      ListHeaderComponent={
        total > 0 ? <Text style={styles.header}>Студентов с заданием: {total}</Text> : undefined
      }
      refreshControl={
        <RefreshControl
          refreshing={q.isRefetching && !q.isFetchingNextPage}
          onRefresh={() => void q.refetch()}
        />
      }
      onEndReached={() => q.hasNextPage && !q.isFetchingNextPage && q.fetchNextPage()}
      onEndReachedThreshold={0.4}
      ListEmptyComponent={q.isLoading ? undefined : <Empty text="Невыполненных заданий нет" />}
      renderItem={({ item: u }) => (
        <TouchableOpacity style={styles.item} onPress={() => router.push(`/student/${u.id}`)}>
          <View style={{ flex: 1 }}>
            <Text style={styles.title}>{u.full_name}</Text>
            <Text style={styles.sub}>{u.group_name ?? 'без группы'}</Text>
          </View>
          <Text style={styles.debt}>{fmtMinutes(u.debt_minutes)}</Text>
          <Icon name="chevron" color={colors.muted} />
        </TouchableOpacity>
      )}
    />
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  header: { fontSize: 13, color: colors.muted, marginBottom: 8 },
  item: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: colors.card,
    borderRadius: 12,
    padding: 14,
    marginBottom: 8,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: colors.border,
  },
  title: { fontSize: 15, fontWeight: '600', color: colors.text },
  sub: { fontSize: 12, color: colors.muted, marginTop: 2 },
  debt: { fontSize: 15, fontWeight: '700', color: colors.danger, marginHorizontal: 6 },
}));
