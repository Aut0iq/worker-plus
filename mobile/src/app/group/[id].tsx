import { useState } from 'react';
import { FlatList, RefreshControl, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { router, useLocalSearchParams } from 'expo-router';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { UserDto } from '@/api/types/UserDto';
import { fmtMinutes } from '@/format';
import { Empty, Field } from '@/components/ui';
import { colors } from '@/theme';

export default function GroupStudents() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const [filter, setFilter] = useState('');
  const q = useQuery({
    queryKey: ['group-students', id],
    queryFn: () => api.get<UserDto[]>(`/api/groups/${id}/students`),
  });
  const items = (q.data ?? []).filter((u) =>
    u.full_name.toLowerCase().includes(filter.trim().toLowerCase()),
  );
  return (
    <View style={{ flex: 1, backgroundColor: colors.bg }}>
      <View style={{ padding: 12, paddingBottom: 0 }}>
        <Field label="Поиск" value={filter} onChangeText={setFilter} placeholder="Фамилия или имя" />
      </View>
      <FlatList
        contentContainerStyle={{ padding: 12 }}
        data={items}
        keyExtractor={(u) => u.id}
        refreshControl={<RefreshControl refreshing={q.isRefetching} onRefresh={() => void q.refetch()} />}
        ListEmptyComponent={q.isLoading ? undefined : <Empty text="Студентов нет" />}
        renderItem={({ item: u }) => (
          <TouchableOpacity style={styles.item} onPress={() => router.push(`/student/${u.id}`)}>
            <View style={{ flex: 1 }}>
              <Text style={styles.title}>{u.full_name}</Text>
              <Text style={styles.sub}>
                долг <Text style={{ color: colors.danger }}>{fmtMinutes(u.debt_minutes)}</Text>
                {'  ·  '}баланс{' '}
                <Text style={{ color: colors.success }}>{fmtMinutes(u.balance_minutes)}</Text>
              </Text>
            </View>
            <Text style={styles.arrow}>›</Text>
          </TouchableOpacity>
        )}
      />
    </View>
  );
}

const styles = StyleSheet.create({
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
  arrow: { fontSize: 22, color: colors.muted, marginLeft: 8 },
});
