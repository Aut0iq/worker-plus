import { Card, Col, List, Row, Statistic, Typography } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import dayjs from 'dayjs';
import { api } from '../api/client';
import { fmtTs } from '../format';
import type { Paginated } from '../api/types/Paginated';
import type { SessionDto } from '../api/types/SessionDto';
import type { OrderDto } from '../api/types/OrderDto';
import type { NotificationDto } from '../api/types/NotificationDto';

export default function Dashboard() {
  const cnt = (p: string) => api.get<Paginated<SessionDto>>(p).then((r) => Number(r.total));
  const { data: active } = useQuery({
    queryKey: ['dash-active'],
    queryFn: () => cnt('/api/admin/sessions?status=active&per_page=1'),
    refetchInterval: 30_000,
  });
  const { data: pending } = useQuery({
    queryKey: ['dash-photo'],
    queryFn: () => cnt('/api/admin/sessions?photo_review=pending&per_page=1'),
  });
  const { data: orders } = useQuery({
    queryKey: ['dash-orders'],
    queryFn: () =>
      api.get<Paginated<OrderDto>>('/api/admin/orders?status=pending&per_page=1').then((r) => Number(r.total)),
  });
  const { data: expired } = useQuery({
    queryKey: ['dash-expired'],
    queryFn: () =>
      cnt(
        `/api/admin/sessions?status=expired&from=${encodeURIComponent(dayjs().subtract(7, 'day').toISOString())}&per_page=1`,
      ),
  });
  const { data: notifs } = useQuery({
    queryKey: ['dash-notifs'],
    queryFn: () => api.get<Paginated<NotificationDto>>('/api/notifications?per_page=10'),
  });
  return (
    <>
      <Row gutter={16}>
        <Col span={6}>
          <Card>
            <Statistic title="Идёт ОПТ" value={active ?? 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Фото на проверке" value={pending ?? 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Заказы к выдаче" value={orders ?? 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Просрочено за 7 дней" value={expired ?? 0} />
          </Card>
        </Col>
      </Row>
      <Card title="Последние уведомления" style={{ marginTop: 16 }}>
        <List
          dataSource={notifs?.items ?? []}
          locale={{ emptyText: 'Нет уведомлений' }}
          renderItem={(n) => (
            <List.Item>
              <List.Item.Meta
                title={
                  <Link to="#">
                    <Typography.Text strong={!n.read_at}>{n.title}</Typography.Text>
                  </Link>
                }
                description={n.body}
              />
              <Typography.Text type="secondary">{fmtTs(n.created_at)}</Typography.Text>
            </List.Item>
          )}
        />
      </Card>
    </>
  );
}
