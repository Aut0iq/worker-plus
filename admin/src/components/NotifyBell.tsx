import { Badge, Button, Dropdown, List, Typography } from 'antd';
import { BellOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import { fmtTs } from '../format';
import type { Paginated } from '../api/types/Paginated';
import type { NotificationDto } from '../api/types/NotificationDto';

export default function NotifyBell() {
  const qc = useQueryClient();
  const { data } = useQuery({
    queryKey: ['notifications'],
    queryFn: () => api.get<Paginated<NotificationDto>>('/api/notifications?per_page=20'),
    refetchInterval: 30_000,
  });
  const items = data?.items ?? [];
  const unread = items.filter((x) => !x.read_at).length;
  const mut = useMutation({
    mutationFn: (id: string) => api.post(`/api/notifications/${id}/read`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['notifications'] }),
  });
  const mutAll = useMutation({
    mutationFn: () => api.post('/api/notifications/read-all'),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['notifications'] }),
  });
  return (
    <Dropdown
      trigger={['click']}
      dropdownRender={() => (
        <div
          style={{
            background: '#fff',
            width: 380,
            maxHeight: 400,
            overflow: 'auto',
            borderRadius: 8,
            boxShadow: '0 6px 16px rgba(0,0,0,.12)',
          }}
        >
          <div style={{ padding: '8px 16px', textAlign: 'right', borderBottom: '1px solid #f0f0f0' }}>
            <Button size="small" type="link" onClick={() => mutAll.mutate()}>
              Прочитать все
            </Button>
          </div>
          <List
            dataSource={items}
            locale={{ emptyText: 'Нет уведомлений' }}
            renderItem={(n) => (
              <List.Item
                style={{ padding: '8px 16px', opacity: n.read_at ? 0.6 : 1, cursor: 'pointer' }}
                onClick={() => !n.read_at && mut.mutate(n.id)}
              >
                <List.Item.Meta
                  title={
                    <Typography.Text strong={!n.read_at} style={{ fontSize: 13 }}>
                      {n.title}
                    </Typography.Text>
                  }
                  description={
                    <>
                      <div style={{ fontSize: 12 }}>{n.body}</div>
                      <Typography.Text type="secondary" style={{ fontSize: 11 }}>
                        {fmtTs(n.created_at)}
                      </Typography.Text>
                    </>
                  }
                />
              </List.Item>
            )}
          />
        </div>
      )}
    >
      <Badge count={unread} size="small">
        <Button type="text" icon={<BellOutlined style={{ fontSize: 18 }} />} />
      </Badge>
    </Dropdown>
  );
}
