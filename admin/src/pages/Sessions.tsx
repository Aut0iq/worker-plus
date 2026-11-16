import { Button, DatePicker, Drawer, Image, Input, Modal, Select, Space, Table, Tabs, Tag, message } from 'antd';
import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, downloadBlob } from '../api/client';
import { fmtMinutes, fmtSeconds, fmtTs } from '../format';
import { difficultyLabels, photoReviewLabels, sessionStatusLabels } from '../labels';
import { UserSelect } from '../components/UserSelect';
import type { Paginated } from '../api/types/Paginated';
import type { SessionDto } from '../api/types/SessionDto';
import type { Dayjs } from 'dayjs';

const { RangePicker } = DatePicker;

export default function SessionsPage() {
  const [tab, setTab] = useState('list');
  return (
    <Tabs
      activeKey={tab}
      onChange={setTab}
      items={[
        { key: 'list', label: 'Все отработки', children: <SessionList /> },
        { key: 'review', label: 'Проверка фото', children: <PhotoReview /> },
      ]}
    />
  );
}

function useFilters() {
  const [status, setStatus] = useState<string>();
  const [review, setReview] = useState<string>();
  const [student, setStudent] = useState<string>();
  const [komendant, setKomendant] = useState<string>();
  const [range, setRange] = useState<[Dayjs | null, Dayjs | null] | null>(null);
  const qs = () => {
    const p = new URLSearchParams({ per_page: '20' });
    if (status) p.set('status', status);
    if (review) p.set('photo_review', review);
    if (student) p.set('student_id', student);
    if (komendant) p.set('komendant_id', komendant);
    if (range?.[0]) p.set('from', range[0].toISOString());
    if (range?.[1]) p.set('to', range[1].toISOString());
    return p;
  };
  return { status, setStatus, review, setReview, student, setStudent, komendant, setKomendant, range, setRange, qs };
}

function SessionList() {
  const f = useFilters();
  const [page, setPage] = useState(1);
  const { data, isLoading } = useQuery({
    queryKey: ['sessions', page, f.qs().toString()],
    queryFn: () => {
      const p = f.qs();
      p.set('page', String(page));
      return api.get<Paginated<SessionDto>>(`/api/admin/sessions?${p}`);
    },
  });
  return (
    <>
      <Space wrap style={{ marginBottom: 12 }}>
        <Select
          allowClear
          placeholder="Статус"
          style={{ width: 210 }}
          onChange={(v) => { setPage(1); f.setStatus(v); }}
          options={Object.entries(sessionStatusLabels).map(([v, x]) => ({ value: v, label: x.label }))}
        />
        <Select
          allowClear
          placeholder="Проверка фото"
          style={{ width: 160 }}
          onChange={(v) => { setPage(1); f.setReview(v); }}
          options={Object.entries(photoReviewLabels).map(([v, x]) => ({ value: v, label: x.label }))}
        />
        <UserSelect role="student" value={f.student} onChange={(v) => { setPage(1); f.setStudent(v); }} placeholder="Студент" />
        <UserSelect role="komendant" value={f.komendant} onChange={(v) => { setPage(1); f.setKomendant(v); }} placeholder="Комендант" />
        <RangePicker showTime onChange={(v) => { setPage(1); f.setRange(v); }} />
      </Space>
      <SessionTable data={data} loading={isLoading} page={page} onPage={setPage} />
    </>
  );
}

function SessionTable({
  data,
  loading,
  page,
  onPage,
}: {
  data?: Paginated<SessionDto>;
  loading: boolean;
  page: number;
  onPage: (p: number) => void;
}) {
  return (
    <Table
      rowKey="id"
      size="small"
      loading={loading}
      dataSource={data?.items ?? []}
      pagination={{ current: page, pageSize: 20, total: Number(data?.total ?? 0), onChange: onPage }}
      columns={[
        { title: 'Студент', dataIndex: 'student_name' },
        { title: 'Группа', dataIndex: 'group_name', render: (v) => v ?? '—' },
        { title: 'Комендант', dataIndex: 'komendant_name' },
        { title: 'Сложность', render: (_, s) => difficultyLabels[s.difficulty] ?? s.difficulty },
        { title: 'Начало', render: (_, s) => fmtTs(s.started_at) },
        { title: 'Конец', render: (_, s) => fmtTs(s.ended_at) },
        { title: 'Длит.', render: (_, s) => fmtSeconds(s.raw_seconds) },
        { title: 'Округл.', render: (_, s) => fmtMinutes(s.rounded_minutes) },
        { title: 'Зачтено', render: (_, s) => fmtMinutes(s.credited_minutes) },
        {
          title: 'Статус',
          render: (_, s) => {
            const st = sessionStatusLabels[s.status];
            return <Tag color={st?.color}>{st?.label ?? s.status}</Tag>;
          },
        },
        {
          title: 'Фото',
          render: (_, s) => {
            if (!s.has_photo) return '—';
            const r = s.photo_review ? photoReviewLabels[s.photo_review] : null;
            return r ? <Tag color={r.color}>{r.label}</Tag> : '—';
          },
        },
      ]}
    />
  );
}

function PhotoReview() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [page, setPage] = useState(1);
  const [current, setCurrent] = useState<SessionDto | null>(null);
  const { data, isLoading } = useQuery({
    queryKey: ['photo-review', page],
    queryFn: () =>
      api.get<Paginated<SessionDto>>(`/api/admin/sessions?photo_review=pending&page=${page}&per_page=20`),
  });
  return (
    <>
      {ctx}
      <Table
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={data?.items ?? []}
        pagination={{ current: page, pageSize: 20, total: Number(data?.total ?? 0), onChange: setPage }}
        locale={{ emptyText: 'Нет фото на проверке' }}
        columns={[
          { title: 'Студент', dataIndex: 'student_name' },
          { title: 'Группа', dataIndex: 'group_name' },
          { title: 'Комендант', dataIndex: 'komendant_name' },
          { title: 'Начало', render: (_, s) => fmtTs(s.started_at) },
          { title: 'Конец', render: (_, s) => fmtTs(s.ended_at) },
          { title: 'Зачтено', render: (_, s) => fmtMinutes(s.credited_minutes) },
          { title: '', render: (_, s) => <Button onClick={() => setCurrent(s)}>Проверить</Button> },
        ]}
      />
      <ReviewDrawer
        session={current}
        onClose={() => setCurrent(null)}
        onDone={() => {
          setCurrent(null);
          qc.invalidateQueries({ queryKey: ['photo-review'] });
          qc.invalidateQueries({ queryKey: ['sessions'] });
          msg.success('Готово');
        }}
      />
    </>
  );
}

function ReviewDrawer({
  session,
  onClose,
  onDone,
}: {
  session: SessionDto | null;
  onClose: () => void;
  onDone: () => void;
}) {
  const [msg, ctx] = message.useMessage();
  const [photoUrl, setPhotoUrl] = useState<string>();
  useEffect(() => {
    setPhotoUrl(undefined);
    if (!session?.has_photo) return;
    let alive = true;
    let url: string | undefined;
    downloadBlob(`/api/admin/sessions/${session.id}/photo`)
      .then((u) => {
        if (alive) {
          url = u;
          setPhotoUrl(u);
        } else {
          URL.revokeObjectURL(u);
        }
      })
      .catch((e) => msg.error(e.message));
    return () => {
      alive = false;
      if (url) URL.revokeObjectURL(url);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session?.id]);

  const review = useMutation({
    mutationFn: (v: { approved: boolean; comment?: string }) =>
      api.post(`/api/admin/sessions/${session!.id}/photo-review`, {
        approved: v.approved,
        comment: v.comment ?? null,
      }),
    onSuccess: onDone,
    onError: (e) => msg.error((e as Error).message),
  });

  const reject = () => {
    let comment = '';
    Modal.confirm({
      title: 'Не зачесть фото?',
      content: (
        <>
          <p>Зачтённые часы будут сняты: часть с баланса, остаток — в долг студента.</p>
          <Input.TextArea
            placeholder="Комментарий (обязательно)"
            onChange={(e) => {
              comment = e.target.value;
            }}
          />
        </>
      ),
      okText: 'Не зачесть',
      okButtonProps: { danger: true },
      onOk: () => {
        if (!comment.trim()) {
          msg.error('Комментарий обязателен');
          return Promise.reject();
        }
        return review.mutateAsync({ approved: false, comment });
      },
    });
  };

  return (
    <Drawer title="Проверка фото" open={!!session} onClose={onClose} width={640}>
      {ctx}
      {session && (
        <>
          {photoUrl ? <Image src={photoUrl} style={{ maxHeight: 360 }} /> : <p>Загрузка фото…</p>}
          <p>
            <b>{session.student_name}</b> ({session.group_name ?? '—'}) · комендант:{' '}
            {session.komendant_name ?? '—'}
            <br />
            {fmtTs(session.started_at)} — {fmtTs(session.ended_at)} ·{' '}
            {difficultyLabels[session.difficulty]} · зачтено:{' '}
            <b>{fmtMinutes(session.credited_minutes)}</b>
            <br />
            На долг: {fmtMinutes(session.applied_to_debt)} · на баланс:{' '}
            {fmtMinutes(session.applied_to_balance)}
          </p>
          <Space>
            <Button type="primary" loading={review.isPending} onClick={() => review.mutate({ approved: true })}>
              Зачесть
            </Button>
            <Button danger loading={review.isPending} onClick={reject}>
              Не зачесть
            </Button>
          </Space>
        </>
      )}
    </Drawer>
  );
}
