import {
  Button,
  Drawer,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
  Upload,
  message,
} from 'antd';
import { DownloadOutlined, UploadOutlined } from '@ant-design/icons';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import { fmtMinutes, fmtTs } from '../format';
import { roleLabels, sessionStatusLabels } from '../labels';
import { HoursInput } from '../components/HoursInput';
import type { Paginated } from '../api/types/Paginated';
import type { UserDto } from '../api/types/UserDto';
import type { GroupDto } from '../api/types/GroupDto';
import type { StudentProfileDto } from '../api/types/StudentProfileDto';
import type { ImportResponse } from '../api/types/ImportResponse';
import type { OrderDto } from '../api/types/OrderDto';
import type { Role } from '../api/types/Role';
import type { UploadFile } from 'antd';
import { orderStatusLabels } from '../labels';

export default function UsersPage() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState('');
  const [role, setRole] = useState<Role>();
  const [groupId, setGroupId] = useState<string>();
  const [active, setActive] = useState<boolean>();
  const [editing, setEditing] = useState<UserDto | null>(null);
  const [creating, setCreating] = useState(false);
  const [detail, setDetail] = useState<string | null>(null);
  const [adjusting, setAdjusting] = useState<UserDto | null>(null);
  const [form] = Form.useForm();
  const [adjustForm] = Form.useForm();

  const qs = () => {
    const p = new URLSearchParams({ page: String(page), per_page: '20' });
    if (search) p.set('q', search);
    if (role) p.set('role', role);
    if (groupId) p.set('group_id', groupId);
    if (active !== undefined) p.set('is_active', String(active));
    return p.toString();
  };
  const { data, isLoading } = useQuery({
    queryKey: ['users', page, search, role, groupId, active],
    queryFn: () => api.get<Paginated<UserDto>>(`/api/admin/users?${qs()}`),
  });
  const { data: groups } = useQuery({
    queryKey: ['groups'],
    queryFn: () => api.get<Paginated<GroupDto> | GroupDto[]>('/api/groups'),
  });
  const groupItems: GroupDto[] = Array.isArray(groups) ? groups : (groups?.items ?? []);

  const saveUser = useMutation({
    mutationFn: async (v: Record<string, unknown>) => {
      if (editing) {
        return api.patch(`/api/admin/users/${editing.id}`, {
          role: v.role ?? null,
          login: v.login ?? null,
          full_name: v.full_name ?? null,
          group_id: v.group_id === undefined ? null : v.group_id,
          is_active: v.is_active ?? null,
        });
      }
      const r = await api.post<{ user: UserDto; generated_password: string | null }>(
        '/api/admin/users',
        { ...v, password: v.password || null },
      );
      if (r.generated_password) {
        Modal.info({
          title: 'Пользователь создан',
          content: (
            <>
              Пароль: <Typography.Text copyable code>{r.generated_password}</Typography.Text>
            </>
          ),
        });
      }
      return r;
    },
    onSuccess: () => {
      msg.success('Сохранено');
      setCreating(false);
      setEditing(null);
      form.resetFields();
      qc.invalidateQueries({ queryKey: ['users'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });

  const resetPassword = (u: UserDto) =>
    Modal.confirm({
      title: `Сбросить пароль ${u.full_name}?`,
      onOk: async () => {
        const r = await api.post<{ password: string }>(`/api/admin/users/${u.id}/reset-password`);
        Modal.info({
          title: 'Новый пароль (показан один раз)',
          content: <Typography.Text copyable code>{r.password}</Typography.Text>,
        });
      },
    });

  const doAdjust = useMutation({
    mutationFn: (v: { debt: number; balance: number; reason: string }) =>
      api.post(`/api/admin/users/${adjusting!.id}/adjust`, {
        debt_delta: Math.trunc(v.debt ?? 0),
        balance_delta: Math.trunc(v.balance ?? 0),
        reason: v.reason,
      }),
    onSuccess: () => {
      msg.success('Скорректировано');
      setAdjusting(null);
      qc.invalidateQueries({ queryKey: ['users'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });

  const doImport = async (file: UploadFile) => {
    const fd = new FormData();
    fd.append('file', file as unknown as Blob);
    try {
      const r = await api.post<ImportResponse>('/api/admin/users/import', fd);
      Modal.info({
        title: 'Результат импорта',
        width: 720,
        content: (
          <Table
            size="small"
            rowKey={(x) => x.line}
            pagination={false}
            dataSource={r.results}
            columns={[
              { title: 'Строка', dataIndex: 'line', width: 60 },
              { title: 'Логин', dataIndex: 'login' },
              {
                title: 'Статус',
                render: (_, x) => (x.ok ? <Tag color="green">OK</Tag> : <Tag color="red">{x.error}</Tag>),
              },
              {
                title: 'Пароль',
                render: (_, x) =>
                  x.generated_password ? <Typography.Text copyable code>{x.generated_password}</Typography.Text> : '—',
              },
            ]}
          />
        ),
      });
      const ok = r.results.filter((x) => x.ok && x.generated_password);
      if (ok.length) {
        const csv = 'Логин,Пароль\n' + ok.map((x) => `${x.login},${x.generated_password}`).join('\n');
        const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv' }));
        const a = document.createElement('a');
        a.href = url;
        a.download = 'credentials.csv';
        a.click();
      }
      qc.invalidateQueries({ queryKey: ['users'] });
    } catch (e) {
      msg.error((e as Error).message);
    }
  };

  const template = () => {
    const csv = 'ФИО,Логин,Пароль,Роль,Группа\nИванов Иван Иванович,ivanov,,студент,1А';
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([csv], { type: 'text/csv' }));
    a.download = 'import-template.csv';
    a.click();
  };

  return (
    <>
      {ctx}
      <Space style={{ marginBottom: 12 }} wrap>
        <Input.Search
          placeholder="ФИО или логин"
          allowClear
          onSearch={(v) => {
            setPage(1);
            setSearch(v);
          }}
          style={{ width: 240 }}
        />
        <Select
          allowClear
          placeholder="Роль"
          style={{ width: 160 }}
          onChange={(v) => {
            setPage(1);
            setRole(v);
          }}
          options={Object.entries(roleLabels).map(([value, label]) => ({ value, label }))}
        />
        <Select
          allowClear
          placeholder="Группа"
          style={{ width: 160 }}
          onChange={(v) => {
            setPage(1);
            setGroupId(v);
          }}
          options={groupItems.map((g) => ({ value: g.id, label: g.name }))}
        />
        <Select
          allowClear
          placeholder="Активность"
          style={{ width: 140 }}
          onChange={(v) => {
            setPage(1);
            setActive(v);
          }}
          options={[
            { value: 'true', label: 'Активные' },
            { value: 'false', label: 'Неактивные' },
          ]}
        />
        <Button type="primary" onClick={() => { setEditing(null); form.resetFields(); setCreating(true); }}>
          Создать
        </Button>
        <Upload accept=".csv,.xlsx" showUploadList={false} beforeUpload={(f) => { doImport(f); return false; }}>
          <Button icon={<UploadOutlined />}>Импорт</Button>
        </Upload>
        <Button icon={<DownloadOutlined />} onClick={template}>Шаблон</Button>
      </Space>
      <Table
        rowKey="id"
        loading={isLoading}
        dataSource={data?.items ?? []}
        pagination={{
          current: page,
          pageSize: 20,
          total: Number(data?.total ?? 0),
          onChange: setPage,
        }}
        columns={[
          {
            title: 'ФИО',
            render: (_, u) => <a onClick={() => setDetail(u.id)}>{u.full_name}</a>,
          },
          { title: 'Логин', dataIndex: 'login' },
          { title: 'Роль', render: (_, u) => roleLabels[u.role] ?? u.role },
          { title: 'Группа', dataIndex: 'group_name', render: (v) => v ?? '—' },
          { title: 'Долг', render: (_, u) => fmtMinutes(u.debt_minutes) },
          { title: 'Баланс', render: (_, u) => fmtMinutes(u.balance_minutes) },
          {
            title: 'Активен',
            render: (_, u) => (u.is_active ? <Tag color="green">Да</Tag> : <Tag color="red">Нет</Tag>),
          },
          {
            title: '',
            render: (_, u) => (
              <Space>
                <a onClick={() => { setCreating(true); setEditing(u); form.setFieldsValue({ ...u, password: undefined }); }}>Изм.</a>
                <a onClick={() => resetPassword(u)}>Пароль</a>
                <a onClick={() => { setAdjusting(u); adjustForm.resetFields(); }}>Корр.</a>
              </Space>
            ),
          },
        ]}
      />

      <Modal
        title={editing ? 'Редактировать пользователя' : 'Создать пользователя'}
        open={creating}
        onCancel={() => { setCreating(false); setEditing(null); }}
        onOk={() => form.submit()}
        confirmLoading={saveUser.isPending}
      >
        <Form form={form} layout="vertical" onFinish={(v) => saveUser.mutate(v)}>
          <Form.Item name="full_name" label="ФИО" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="login" label="Логин" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          {!editing && (
            <Form.Item name="password" label="Пароль (пусто — сгенерируется)">
              <Input.Password />
            </Form.Item>
          )}
          <Form.Item name="role" label="Роль" rules={[{ required: true }]}>
            <Select options={Object.entries(roleLabels).map(([value, label]) => ({ value, label }))} />
          </Form.Item>
          <Form.Item noStyle shouldUpdate={(a, b) => a.role !== b.role}>
            {({ getFieldValue }) =>
              getFieldValue('role') === 'student' && (
                <Form.Item name="group_id" label="Группа">
                  <Select
                    allowClear
                    options={groupItems.map((g) => ({ value: g.id, label: g.name }))}
                  />
                </Form.Item>
              )
            }
          </Form.Item>
          {editing && (
            <Form.Item name="is_active" label="Активен" valuePropName="checked">
              <Switch />
            </Form.Item>
          )}
        </Form>
      </Modal>

      <Modal
        title={`Корректировка: ${adjusting?.full_name ?? ''}`}
        open={!!adjusting}
        onCancel={() => setAdjusting(null)}
        onOk={() => adjustForm.submit()}
      >
        <Form form={adjustForm} layout="vertical" onFinish={(v) => doAdjust.mutate(v)}>
          <Form.Item name="debt" label="Изменение долга (минуты, может быть отрицательным)">
            <Input type="number" />
          </Form.Item>
          <Form.Item name="balance" label="Изменение баланса (минуты)">
            <Input type="number" />
          </Form.Item>
          <Form.Item name="reason" label="Причина" rules={[{ required: true }]}>
            <Input.TextArea />
          </Form.Item>
        </Form>
      </Modal>

      <UserDetail id={detail} onClose={() => setDetail(null)} />
    </>
  );
}

function UserDetail({ id, onClose }: { id: string | null; onClose: () => void }) {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [form] = Form.useForm();
  const { data } = useQuery({
    queryKey: ['student', id],
    enabled: !!id,
    queryFn: () => api.get<StudentProfileDto>(`/api/students/${id}`),
  });
  const { data: orders } = useQuery({
    queryKey: ['user-orders', id],
    enabled: !!id,
    queryFn: () => api.get<Paginated<OrderDto>>(`/api/admin/orders?user_id=${id}`).catch(() => null),
  });
  const assign = useMutation({
    mutationFn: (v: { minutes: number; reason: string }) =>
      api.post(`/api/students/${id}/assignments`, { minutes: v.minutes, reason: v.reason }),
    onSuccess: () => {
      msg.success('Часы назначены');
      form.resetFields();
      qc.invalidateQueries({ queryKey: ['student', id] });
      qc.invalidateQueries({ queryKey: ['users'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });
  const u = data?.user;
  return (
    <Drawer title={u?.full_name ?? '…'} open={!!id} onClose={onClose} width={720}>
      {ctx}
      {u && (
        <>
          <Typography.Paragraph>
            Логин: {u.login} · Роль: {roleLabels[u.role] ?? u.role} · Группа: {u.group_name ?? '—'}
            <br />
            Долг: <b>{fmtMinutes(u.debt_minutes)}</b> · Баланс: <b>{fmtMinutes(u.balance_minutes)}</b>
          </Typography.Paragraph>
          {u.role === 'student' && (
            <Form form={form} layout="inline" onFinish={(v) => assign.mutate(v)} style={{ marginBottom: 16 }}>
              <Form.Item name="minutes" label="Назначить часы" rules={[{ required: true }]}>
                <HoursInput value={form.getFieldValue('minutes')} onChange={(v) => form.setFieldValue('minutes', v)} />
              </Form.Item>
              <Form.Item name="reason" rules={[{ required: true }]}>
                <Input placeholder="Причина" style={{ width: 200 }} />
              </Form.Item>
              <Button type="primary" htmlType="submit">Назначить</Button>
            </Form>
          )}
          <Typography.Title level={5}>Отработки</Typography.Title>
          <Table
            size="small"
            rowKey="id"
            dataSource={data?.recent_sessions ?? []}
            pagination={false}
            columns={[
              { title: 'Начало', render: (_, s) => fmtTs(s.started_at) },
              { title: 'Конец', render: (_, s) => fmtTs(s.ended_at) },
              { title: 'Зачтено', render: (_, s) => fmtMinutes(s.credited_minutes) },
              {
                title: 'Статус',
                render: (_, s) => {
                  const st = sessionStatusLabels[s.status];
                  return <Tag color={st?.color}>{st?.label ?? s.status}</Tag>;
                },
              },
            ]}
          />
          <Typography.Title level={5} style={{ marginTop: 16 }}>Назначения</Typography.Title>
          <Table
            size="small"
            rowKey="id"
            dataSource={data?.assignments ?? []}
            pagination={false}
            columns={[
              { title: 'Часы', render: (_, a) => fmtMinutes(a.minutes) },
              { title: 'Причина', dataIndex: 'reason' },
              { title: 'Кем', dataIndex: 'assigned_by_name' },
              { title: 'Когда', render: (_, a) => fmtTs(a.created_at) },
            ]}
          />
          {orders?.items && orders.items.length > 0 && (
            <>
              <Typography.Title level={5} style={{ marginTop: 16 }}>Заказы</Typography.Title>
              <Table
                size="small"
                rowKey="id"
                dataSource={orders.items}
                pagination={false}
                columns={[
                  { title: 'Товар', dataIndex: 'reward_title' },
                  { title: 'Цена', render: (_, o) => fmtMinutes(o.price_minutes) },
                  {
                    title: 'Статус',
                    render: (_, o) => {
                      const st = orderStatusLabels[o.status];
                      return <Tag color={st?.color}>{st?.label ?? o.status}</Tag>;
                    },
                  },
                ]}
              />
            </>
          )}
        </>
      )}
    </Drawer>
  );
}
