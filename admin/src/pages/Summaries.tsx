import { Button, DatePicker, Drawer, Modal, Table, Typography, message } from 'antd';
import { DownloadOutlined } from '@ant-design/icons';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import dayjs, { type Dayjs } from 'dayjs';
import { api, downloadFile } from '../api/client';
import { fmtMinutes, fmtTs } from '../format';
import type { Paginated } from '../api/types/Paginated';
import type { SummaryDto } from '../api/types/SummaryDto';
import type { SummaryRowDto } from '../api/types/SummaryRowDto';

const monthName = (m: number) =>
  ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'][m - 1];

export default function SummariesPage() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [detail, setDetail] = useState<SummaryDto | null>(null);
  const { data, isLoading } = useQuery({
    queryKey: ['summaries'],
    queryFn: () => api.get<Paginated<SummaryDto> | SummaryDto[]>('/api/admin/summaries'),
  });
  const items: SummaryDto[] = Array.isArray(data) ? data : (data?.items ?? []);

  const generate = useMutation({
    mutationFn: (v: { year: number; month: number }) =>
      api.post('/api/admin/summaries/generate', v),
    onSuccess: () => {
      msg.success('Сводка сформирована');
      qc.invalidateQueries({ queryKey: ['summaries'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });

  const pickMonth = () => {
    let d: Dayjs | null = dayjs().subtract(1, 'month');
    Modal.confirm({
      title: 'Сформировать сводку',
      content: (
        <div style={{ marginTop: 8 }}>
          <DatePicker picker="month" defaultValue={d} onChange={(v) => (d = v)} />
          <p style={{ marginTop: 8, color: '#888' }}>
            Если сводка за этот месяц уже есть, она будет заменена.
          </p>
        </div>
      ),
      okText: 'Сформировать',
      onOk: () => {
        if (!d) return Promise.reject();
        const existing = items.find((s) => s.year === d!.year() && s.month === d!.month() + 1);
        const doIt = () => generate.mutateAsync({ year: d!.year(), month: d!.month() + 1 });
        if (!existing) return doIt();
        Modal.confirm({
          title: 'Заменить существующую сводку?',
          onOk: () => doIt(),
        });
        return Promise.resolve();
      },
    });
  };

  return (
    <>
      {ctx}
      <Button type="primary" style={{ marginBottom: 12 }} onClick={pickMonth}>
        Сформировать
      </Button>
      <Table
        rowKey="id"
        loading={isLoading}
        dataSource={items}
        pagination={false}
        columns={[
          {
            title: 'Период',
            render: (_, s) => <a onClick={() => setDetail(s)}>{`${monthName(s.month)} ${s.year}`}</a>,
          },
          { title: 'Сформирована', render: (_, s) => fmtTs(s.generated_at) },
          {
            title: '',
            render: (_, s) => (
              <Button
                icon={<DownloadOutlined />}
                onClick={() =>
                  downloadFile(`/api/admin/summaries/${s.id}/xlsx`, `summary-${s.year}-${s.month}.xlsx`).catch(
                    (e) => msg.error(e.message),
                  )
                }
              >
                Excel
              </Button>
            ),
          },
        ]}
      />
      <SummaryDrawer summary={detail} onClose={() => setDetail(null)} />
    </>
  );
}

function SummaryDrawer({ summary, onClose }: { summary: SummaryDto | null; onClose: () => void }) {
  const { data } = useQuery({
    queryKey: ['summary', summary?.id],
    enabled: !!summary,
    queryFn: () => api.get<SummaryDto>(`/api/admin/summaries/${summary!.id}`),
  });
  const rows = data?.rows ?? [];
  const groups = new Map<string, SummaryRowDto[]>();
  rows.forEach((r) => {
    const k = r.group_name ?? 'Без группы';
    groups.set(k, [...(groups.get(k) ?? []), r]);
  });
  const sum = (arr: SummaryRowDto[], f: (r: SummaryRowDto) => number) =>
    arr.reduce((a, r) => a + Number(f(r)), 0);
  const cols = [
    { title: 'Студент', dataIndex: 'full_name' },
    { title: 'Сессий', render: (_: unknown, r: SummaryRowDto) => Number(r.sessions_count) },
    { title: 'Округлено', render: (_: unknown, r: SummaryRowDto) => fmtMinutes(r.rounded_minutes) },
    { title: 'Зачтено', render: (_: unknown, r: SummaryRowDto) => fmtMinutes(r.credited_minutes) },
    { title: 'Назначено', render: (_: unknown, r: SummaryRowDto) => fmtMinutes(r.assigned_minutes) },
    { title: 'Долг', render: (_: unknown, r: SummaryRowDto) => fmtMinutes(r.debt_minutes) },
    { title: 'Баланс', render: (_: unknown, r: SummaryRowDto) => fmtMinutes(r.balance_minutes) },
  ];
  return (
    <Drawer
      title={summary ? `Сводка: ${monthName(summary.month)} ${summary.year}` : ''}
      open={!!summary}
      onClose={onClose}
      width={900}
    >
      {[...groups.entries()].map(([g, arr]) => (
        <div key={g} style={{ marginBottom: 24 }}>
          <Typography.Title level={5}>{g}</Typography.Title>
          <Table
            size="small"
            rowKey="id"
            dataSource={arr}
            pagination={false}
            columns={cols}
            summary={() => (
              <Table.Summary.Row>
                <Table.Summary.Cell index={0}>
                  <b>Итого</b>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1}>{sum(arr, (r) => r.sessions_count)}</Table.Summary.Cell>
                <Table.Summary.Cell index={2}>{fmtMinutes(sum(arr, (r) => r.rounded_minutes))}</Table.Summary.Cell>
                <Table.Summary.Cell index={3}>{fmtMinutes(sum(arr, (r) => r.credited_minutes))}</Table.Summary.Cell>
                <Table.Summary.Cell index={4}>{fmtMinutes(sum(arr, (r) => r.assigned_minutes))}</Table.Summary.Cell>
                <Table.Summary.Cell index={5} />
                <Table.Summary.Cell index={6} />
              </Table.Summary.Row>
            )}
          />
        </div>
      ))}
    </Drawer>
  );
}
