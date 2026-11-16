import { useState } from 'react';
import { Alert, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '@/api/client';
import type { OrderDto } from '@/api/types/OrderDto';
import type { Paginated } from '@/api/types/Paginated';
import type { RewardDto } from '@/api/types/RewardDto';
import type { UserDto } from '@/api/types/UserDto';
import { useAuth } from '@/auth';
import { fmtMinutes, fmtTs } from '@/format';
import { orderStatusLabels } from '@/labels';
import { AuthImage } from '@/components/AuthImage';
import { Button, Card, Empty, ErrorText } from '@/components/ui';
import { colors } from '@/theme';

export default function Shop() {
  const { config } = useAuth();
  const tz = config?.timezone ?? 'UTC';
  const qc = useQueryClient();
  const [tab, setTab] = useState<'shop' | 'orders'>('shop');
  const [err, setErr] = useState<string | null>(null);

  const me = useQuery({ queryKey: ['me'], queryFn: () => api.get<UserDto>('/api/me') });
  const rewards = useQuery({ queryKey: ['rewards'], queryFn: () => api.get<RewardDto[]>('/api/rewards') });
  const orders = useQuery({
    queryKey: ['my-orders'],
    queryFn: () => api.get<Paginated<OrderDto>>('/api/student/orders?page=1&per_page=50'),
    enabled: tab === 'orders',
  });

  const buy = useMutation({
    mutationFn: (id: string) => api.post<OrderDto>(`/api/rewards/${id}/buy`),
    onSuccess: () => {
      setErr(null);
      void qc.invalidateQueries({ queryKey: ['me'] });
      void qc.invalidateQueries({ queryKey: ['my-orders'] });
      void qc.invalidateQueries({ queryKey: ['rewards'] });
      Alert.alert('Готово', 'Заказ создан');
    },
    onError: (e) => setErr(e instanceof ApiError ? e.message : 'Ошибка покупки'),
  });

  const balance = me.data?.balance_minutes ?? 0;

  return (
    <View style={styles.wrap}>
      <View style={styles.tabs}>
        <Text
          style={[styles.tab, tab === 'shop' && styles.tabOn]}
          onPress={() => setTab('shop')}
        >
          Магазин
        </Text>
        <Text
          style={[styles.tab, tab === 'orders' && styles.tabOn]}
          onPress={() => setTab('orders')}
        >
          Мои заказы
        </Text>
        <View style={{ flex: 1 }} />
        <Text style={styles.balance}>Баланс: {fmtMinutes(balance)}</Text>
      </View>
      <ScrollView contentContainerStyle={{ padding: 12 }}>
        <ErrorText>{err}</ErrorText>
        {tab === 'shop' &&
          (rewards.data?.length ? (
            rewards.data.map((r) => {
              const noStock = r.stock === 0;
              const poor = balance < r.price_minutes;
              return (
                <Card key={r.id}>
                  <View style={{ flexDirection: 'row' }}>
                    {r.image_url ? (
                      <AuthImage path={r.image_url} style={styles.img} resizeMode="cover" />
                    ) : null}
                    <View style={{ flex: 1, marginLeft: r.image_url ? 10 : 0 }}>
                      <Text style={styles.title}>{r.title}</Text>
                      {!!r.description && <Text style={styles.sub}>{r.description}</Text>}
                      <Text style={styles.sub}>
                        Цена: {fmtMinutes(r.price_minutes)}
                        {r.stock == null ? ' · безлимит' : ` · осталось: ${r.stock}`}
                      </Text>
                      <Button
                        title={noStock ? 'Нет в наличии' : poor ? 'Не хватает баланса' : 'Купить'}
                        disabled={noStock || poor}
                        loading={buy.isPending && buy.variables === r.id}
                        onPress={() =>
                          Alert.alert('Купить?', `${r.title} за ${fmtMinutes(r.price_minutes)}`, [
                            { text: 'Отмена', style: 'cancel' },
                            { text: 'Купить', onPress: () => buy.mutate(r.id) },
                          ])
                        }
                        style={{ marginTop: 8 }}
                      />
                    </View>
                  </View>
                </Card>
              );
            })
          ) : (
            <Empty text={rewards.isLoading ? 'Загрузка…' : 'Наград пока нет'} />
          ))}
        {tab === 'orders' &&
          (orders.data?.items.length ? (
            orders.data.items.map((o) => {
              const st = orderStatusLabels[o.status] ?? { label: o.status, color: colors.muted };
              return (
                <Card key={o.id}>
                  <View style={{ flexDirection: 'row', alignItems: 'center' }}>
                    <View style={{ flex: 1 }}>
                      <Text style={styles.title}>{o.reward_title ?? 'Награда'}</Text>
                      <Text style={styles.sub}>
                        {fmtMinutes(o.price_minutes)} · {fmtTs(o.created_at, tz)}
                      </Text>
                    </View>
                    <Text style={{ color: st.color, fontWeight: '600' }}>{st.label}</Text>
                  </View>
                </Card>
              );
            })
          ) : (
            <Empty text={orders.isLoading ? 'Загрузка…' : 'Заказов нет'} />
          ))}
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg },
  tabs: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 12,
    paddingTop: 12,
    gap: 16,
  },
  tab: { fontSize: 15, color: colors.muted, paddingBottom: 4 },
  tabOn: { color: colors.primary, fontWeight: '700', borderBottomWidth: 2, borderBottomColor: colors.primary },
  balance: { fontSize: 13, color: colors.muted },
  img: { width: 72, height: 72, borderRadius: 8, backgroundColor: colors.border },
  title: { fontSize: 15, fontWeight: '600', color: colors.text },
  sub: { fontSize: 12, color: colors.muted, marginTop: 3 },
});
