import { DatePicker, Space, Switch, Table, Tabs, Tag, Typography } from 'antd';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { Dayjs } from 'dayjs';
import { api } from '../api/client';
import { fmtTs } from '../format';
import { actionLabel, actionLabels, roleLabels } from '../labels';
import { UserSelect } from '../components/UserSelect';
import type { Paginated } from '../api/types/Paginated';
import type { AuditEntryDto } from '../api/types/AuditEntryDto';
import type { KomendantLogEntryDto } from '../api/types/KomendantLogEntryDto';
import { Select } from 'antd';

const { RangePicker } = DatePicker;

export default function LogsPage() {
  const [auto, setAuto] = useState(false);
  return (
    <>
      <Space style={{ marginBottom: 12 }}>
        <Switch checked={auto} onChange={setAuto} /> Автообновление (30с)
      </Space>
      <Tabs
        items={[
          { key: 'audit', label: 'Общий журнал', children: <AuditLog auto={auto} /> },
          { key: 'komendant', label: 'Журнал комендантов', children: <KomLog auto={auto} /> },
        ]}
      />
    </>
  );
}

const detailsCell = (d: unknown) => (
  <Typography.Paragraph style={{ fontSize: 12, margin: 0 }} ellipsis={{ rows: 2, expandable: true, symbol: 'ещё' }}>
    {JSON.stringify(d)}
  </Typography.Paragraph>
);

function AuditLog({ auto }: { auto: boolean }) {
  const [page, setPage] = useState(1);
  const [actor, setActor] = useState<string>();
  const [action, setAction] = useState<string>();
  const [range, setRange] = useState<[Dayjs | null, Dayjs | null] | null>(null);
  const { data, isLoading } = useQuery({
    queryKey: ['audit', page, actor, action, range?.[0]?.toISOString(), range?.[1]?.toISOString()],
    refetchInterval: auto ? 30_000 : false,
    queryFn: () => {
      const p = new URLSearchParams({ page: String(page), per_page: '20' });
      if (actor) p.set('actor_id', actor);
      if (action) p.set('action', action);
      if (range?.[0]) p.set('from', range[0].toISOString());
      if (range?.[1]) p.set('to', range[1].toISOString());
      return api.get<Paginated<AuditEntryDto>>(`/api/admin/audit-log?${p}`);
    },
  });
  return (
    <>
      <Space wrap style={{ marginBottom: 12 }}>
        <UserSelect value={actor} onChange={(v) => { setPage(1); setActor(v); }} placeholder="Пользователь" />
        <Select
          allowClear
          showSearch
          placeholder="Действие"
          style={{ width: 240 }}
          onChange={(v) => { setPage(1); setAction(v); }}
          options={Object.entries(actionLabels).map(([v, l]) => ({ value: v, label: l }))}
        />
        <RangePicker showTime onChange={(v) => { setPage(1); setRange(v); }} />
      </Space>
      <Table
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={data?.items ?? []}
        pagination={{ current: page, pageSize: 20, total: Number(data?.total ?? 0), onChange: setPage }}
        columns={[
          { title: 'Время', render: (_, e) => fmtTs(e.ts), width: 170 },
          { title: 'Пользователь', render: (_, e) => e.actor_login ?? '—' },
          { title: 'Роль', render: (_, e) => (e.actor_role ? (roleLabels[e.actor_role] ?? e.actor_role) : '—') },
          { title: 'Действие', render: (_, e) => <Tag>{actionLabel(e.action)}</Tag> },
          { title: 'Объект', render: (_, e) => [e.entity_type, e.entity_id].filter(Boolean).join(' ') || '—' },
          { title: 'Детали', render: (_, e) => detailsCell(e.details) },
          { title: 'IP', dataIndex: 'ip', render: (v) => v ?? '—' },
        ]}
      />
    </>
  );
}

function KomLog({ auto }: { auto: boolean }) {
  const [page, setPage] = useState(1);
  const [kom, setKom] = useState<string>();
  const [range, setRange] = useState<[Dayjs | null, Dayjs | null] | null>(null);
  const { data, isLoading } = useQuery({
    queryKey: ['komlog', page, kom, range?.[0]?.toISOString(), range?.[1]?.toISOString()],
    refetchInterval: auto ? 30_000 : false,
    queryFn: () => {
      const p = new URLSearchParams({ page: String(page), per_page: '20' });
      if (kom) p.set('komendant_id', kom);
      if (range?.[0]) p.set('from', range[0].toISOString());
      if (range?.[1]) p.set('to', range[1].toISOString());
      return api.get<Paginated<KomendantLogEntryDto>>(`/api/admin/komendant-log?${p}`);
    },
  });
  return (
    <>
      <Space wrap style={{ marginBottom: 12 }}>
        <UserSelect role="komendant" value={kom} onChange={(v) => { setPage(1); setKom(v); }} placeholder="Комендант" />
        <RangePicker showTime onChange={(v) => { setPage(1); setRange(v); }} />
      </Space>
      <Table
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={data?.items ?? []}
        pagination={{ current: page, pageSize: 20, total: Number(data?.total ?? 0), onChange: setPage }}
        columns={[
          { title: 'Время', render: (_, e) => fmtTs(e.ts), width: 170 },
          { title: 'Комендант', dataIndex: 'komendant_name' },
          { title: 'Действие', render: (_, e) => <Tag>{actionLabel(e.action)}</Tag> },
          {
            title: 'Студент',
            render: (_, e) => (e.details as { student_name?: string })?.student_name ?? '—',
          },
          { title: 'Детали', render: (_, e) => detailsCell(e.details) },
        ]}
      />
    </>
  );
}
