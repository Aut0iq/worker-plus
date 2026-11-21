import {
  Button,
  Checkbox,
  Form,
  Image,
  Input,
  InputNumber,
  Modal,
  Select,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Upload,
  message,
} from 'antd';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import { fmtMinutes, fmtTs } from '../format';
import { orderStatusLabels } from '../labels';
import { HoursInput } from '../components/HoursInput';
import type { Paginated } from '../api/types/Paginated';
import type { RewardDto } from '../api/types/RewardDto';
import type { OrderDto } from '../api/types/OrderDto';

export default function ShopPage() {
  return (
    <Tabs
      items={[
        { key: 'rewards', label: 'Товары', children: <Rewards /> },
        { key: 'orders', label: 'Заказы', children: <Orders /> },
      ]}
    />
  );
}

function Rewards() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<RewardDto | null>(null);
  const [form] = Form.useForm();
  const { data, isLoading } = useQuery({
    queryKey: ['rewards'],
    queryFn: () => api.get<Paginated<RewardDto> | RewardDto[]>('/api/rewards'),
  });
  const items: RewardDto[] = Array.isArray(data) ? data : (data?.items ?? []);

  const save = useMutation({
    mutationFn: (v: { title: string; description?: string; price: number; unlimited: boolean; stock?: number; is_active: boolean }) =>
      editing
        ? api.patch(`/api/admin/rewards/${editing.id}`, {
            title: v.title,
            description: v.description || null,
            price_minutes: v.price,
            stock: v.unlimited ? null : (v.stock ?? 0),
            is_active: v.is_active,
          })
        : api.post<RewardDto>('/api/admin/rewards', {
            title: v.title,
            description: v.description || null,
            price_minutes: v.price,
            stock: v.unlimited ? null : (v.stock ?? 0),
            is_active: v.is_active,
          }),
    onSuccess: () => {
      msg.success('Сохранено');
      setOpen(false);
      qc.invalidateQueries({ queryKey: ['rewards'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });

  const del = (r: RewardDto) =>
    Modal.confirm({
      title: `Удалить «${r.title}»?`,
      onOk: () =>
        api
          .del(`/api/admin/rewards/${r.id}`)
          .then(() => qc.invalidateQueries({ queryKey: ['rewards'] }))
          .catch((e) => msg.error((e as Error).message)),
    });

  const uploadImage = async (r: RewardDto, file: File) => {
    const fd = new FormData();
    fd.append('image', file);
    try {
      await api.post(`/api/admin/rewards/${r.id}/image`, fd);
      qc.invalidateQueries({ queryKey: ['rewards'] });
      msg.success('Картинка загружена');
    } catch (e) {
      msg.error((e as Error).message);
    }
  };

  return (
    <>
      {ctx}
      <Button
        type="primary"
        style={{ marginBottom: 12 }}
        onClick={() => {
          setEditing(null);
          form.resetFields();
          form.setFieldsValue({ is_active: true, unlimited: true, price: 0 });
          setOpen(true);
        }}
      >
        Создать товар
      </Button>
      <Table
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={items}
        pagination={false}
        columns={[
          {
            title: '',
            width: 70,
            render: (_, r) =>
              r.image_url ? <Image src={r.image_url} width={48} height={48} style={{ objectFit: 'cover' }} /> : '—',
          },
          { title: 'Название', dataIndex: 'title' },
          { title: 'Цена', render: (_, r) => fmtMinutes(r.price_minutes) },
          { title: 'Остаток', render: (_, r) => (r.stock == null ? '∞' : Number(r.stock)) },
          {
            title: 'Активен',
            render: (_, r) => (r.is_active ? <Tag color="green">Да</Tag> : <Tag color="red">Нет</Tag>),
          },
          {
            title: '',
            render: (_, r) => (
              <Space>
                <a
                  onClick={() => {
                    setEditing(r);
                    form.setFieldsValue({
                      title: r.title,
                      description: r.description,
                      price: Number(r.price_minutes),
                      unlimited: r.stock == null,
                      stock: r.stock == null ? 0 : Number(r.stock),
                      is_active: r.is_active,
                    });
                    setOpen(true);
                  }}
                >
                  Изм.
                </a>
                <Upload showUploadList={false} beforeUpload={(f) => { uploadImage(r, f); return false; }}>
                  <a>Картинка</a>
                </Upload>
                <a style={{ color: 'red' }} onClick={() => del(r)}>
                  Удалить
                </a>
              </Space>
            ),
          },
        ]}
      />
      <Modal
        title={editing ? 'Редактировать товар' : 'Создать товар'}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={save.isPending}
      >
        <Form form={form} layout="vertical" onFinish={(v) => save.mutate(v)}>
          <Form.Item name="title" label="Название" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="description" label="Описание">
            <Input.TextArea />
          </Form.Item>
          <Form.Item label="Цена" required>
            <Form.Item name="price" noStyle rules={[{ required: true }]}>
              <HoursInput value={form.getFieldValue('price')} onChange={(v) => form.setFieldValue('price', v)} />
            </Form.Item>
          </Form.Item>
          <Form.Item name="unlimited" valuePropName="checked">
            <Checkbox>Без ограничения остатка</Checkbox>
          </Form.Item>
          <Form.Item noStyle shouldUpdate={(a, b) => a.unlimited !== b.unlimited}>
            {({ getFieldValue }) =>
              !getFieldValue('unlimited') && (
                <Form.Item name="stock" label="Остаток" rules={[{ required: true }]}>
                  <InputNumber min={0} />
                </Form.Item>
              )
            }
          </Form.Item>
          <Form.Item name="is_active" label="Активен" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}

function Orders() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [status, setStatus] = useState<string>();
  const [page, setPage] = useState(1);
  const { data, isLoading } = useQuery({
    queryKey: ['orders', status, page],
    queryFn: () =>
      api.get<Paginated<OrderDto>>(
        `/api/admin/orders?page=${page}&per_page=20${status ? `&status=${status}` : ''}`,
      ),
  });
  const act = useMutation({
    mutationFn: ({ id, op }: { id: string; op: 'issue' | 'cancel' }) =>
      api.post(`/api/admin/orders/${id}/${op}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['orders'] }),
    onError: (e) => msg.error((e as Error).message),
  });
  return (
    <>
      {ctx}
      <Select
        allowClear
        placeholder="Статус"
        style={{ width: 180, marginBottom: 12 }}
        onChange={(v) => {
          setPage(1);
          setStatus(v);
        }}
        options={Object.entries(orderStatusLabels).map(([v, x]) => ({ value: v, label: x.label }))}
      />
      <Table
        rowKey="id"
        size="small"
        loading={isLoading}
        dataSource={data?.items ?? []}
        pagination={{ current: page, pageSize: 20, total: Number(data?.total ?? 0), onChange: setPage }}
        columns={[
          { title: 'Студент', dataIndex: 'user_name' },
          { title: 'Товар', dataIndex: 'reward_title' },
          { title: 'Цена', render: (_, o) => fmtMinutes(o.price_minutes) },
          {
            title: 'Статус',
            render: (_, o) => {
              const st = orderStatusLabels[o.status];
              return <Tag color={st?.color}>{st?.label ?? o.status}</Tag>;
            },
          },
          { title: 'Создан', render: (_, o) => fmtTs(o.created_at) },
          { title: 'Выдан', render: (_, o) => fmtTs(o.issued_at) },
          {
            title: '',
            render: (_, o) =>
              o.status === 'pending' && (
                <Space>
                  <a onClick={() => act.mutate({ id: o.id, op: 'issue' })}>Выдать</a>
                  <a
                    style={{ color: 'red' }}
                    onClick={() =>
                      Modal.confirm({
                        title: 'Отменить заказ? Баланс будет возвращён.',
                        onOk: () => act.mutateAsync({ id: o.id, op: 'cancel' }),
                      })
                    }
                  >
                    Отменить
                  </a>
                </Space>
              ),
          },
        ]}
      />
    </>
  );
}
