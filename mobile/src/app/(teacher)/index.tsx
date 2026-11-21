import { FlatList, RefreshControl, StyleSheet, Text, TouchableOpacity } from 'react-native';
import { router } from 'expo-router';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { GroupDto } from '@/api/types/GroupDto';
import { Icon } from '@/components/Icon';
import { Empty } from '@/components/ui';
import { makeStyles, useColors } from '@/theme';

export default function TeacherGroups() {
  const colors = useColors();
  const styles = useStyles();
  const q = useQuery({ queryKey: ['groups'], queryFn: () => api.get<GroupDto[]>('/api/groups') });
  return (
    <FlatList
      style={{ flex: 1, backgroundColor: colors.bg }}
      contentContainerStyle={{ padding: 12 }}
      data={q.data ?? []}
      keyExtractor={(g) => g.id}
      refreshControl={<RefreshControl refreshing={q.isRefetching} onRefresh={() => void q.refetch()} />}
      ListEmptyComponent={q.isLoading ? undefined : <Empty text="Групп нет" />}
      renderItem={({ item }) => (
        <TouchableOpacity style={styles.item} onPress={() => router.push(`/group/${item.id}`)}>
          <Text style={styles.title}>{item.name}</Text>
          <Text style={styles.count}>{item.student_count} студ.</Text>
          <Icon name="chevron" color={colors.muted} />
        </TouchableOpacity>
      )}
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
    padding: 16,
    marginBottom: 8,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: colors.border,
  },
  title: { flex: 1, fontSize: 16, fontWeight: '600', color: colors.text },
  count: { fontSize: 13, color: colors.muted, marginRight: 4 },
}));
