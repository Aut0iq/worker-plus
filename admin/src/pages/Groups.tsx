import { Button, Drawer, Form, Input, Modal, Space, Table, Tag, message } from 'antd';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import { fmtMinutes, fmtTs } from '../format';
import type { Paginated } from '../api/types/Paginated';
import type { GroupDto } from '../api/types/GroupDto';
import type { UserDto } from '../api/types/UserDto';

export default function GroupsPage() {
  const qc = useQueryClient();
  const [msg, ctx] = message.useMessage();
  const [editing, setEditing] = useState<GroupDto | null>(null);
  const [open, setOpen] = useState(false);
  const [studentsOf, setStudentsOf] = useState<GroupDto | null>(null);
  const [form] = Form.useForm();
  const { data, isLoading } = useQuery({
    queryKey: ['groups'],
    queryFn: () => api.get<Paginated<GroupDto> | GroupDto[]>('/api/groups'),
  });
  const items: GroupDto[] = Array.isArray(data) ? data : (data?.items ?? []);
  const { data: allStudents } = useQuery({
    queryKey: ['all-students'],
    queryFn: () => api.get<Paginated<UserDto>>('/api/admin/users?role=student&per_page=500'),
  });
  const counts = new Map<string, number>();
  allStudents?.items.forEach((u) => {
    if (u.group_id) counts.set(u.group_id, (counts.get(u.group_id) ?? 0) + 1);
  });
  const { data: students } = useQuery({
    queryKey: ['group-students', studentsOf?.id],
    enabled: !!studentsOf,
    queryFn: () => api.get<Paginated<UserDto> | UserDto[]>(`/api/groups/${studentsOf!.id}/students`),
  });
  const studentItems: (UserDto & { student_count?: number })[] = Array.isArray(students)
    ? students
    : (students?.items ?? []);

  const save = useMutation({
    mutationFn: (v: { name: string }) =>
      editing ? api.patch(`/api/admin/groups/${editing.id}`, v) : api.post('/api/admin/groups', v),
    onSuccess: () => {
      msg.success('Сохранено');
      setOpen(false);
      setEditing(null);
      form.resetFields();
      qc.invalidateQueries({ queryKey: ['groups'] });
    },
    onError: (e) => msg.error((e as Error).message),
  });
  const del = (g: GroupDto) =>
    Modal.confirm({
      title: `Удалить группу «${g.name}»?`,
      onOk: () =>
        api
          .del(`/api/admin/groups/${g.id}`)
          .then(() => qc.invalidateQueries({ queryKey: ['groups'] }))
          .catch((e) => msg.error(e.message)),
    });

  return (
    <>
      {ctx}
      <Button
        type="primary"
        style={{ marginBottom: 12 }}
        onClick={() => {
          setEditing(null);
          form.resetFields();
          setOpen(true);
        }}
      >
        Создать группу
      </Button>
      <Table
        rowKey="id"
        loading={isLoading}
        dataSource={items}
        pagination={false}
        columns={[
          {
            title: 'Название',
            render: (_, g) => <a onClick={() => setStudentsOf(g)}>{g.name}</a>,
          },
          {
            title: 'Студентов',
            render: (_, g) => counts.get(g.id) ?? 0,
          },
          { title: 'Создана', render: (_, g) => fmtTs(g.created_at) },
          {
            title: '',
            render: (_, g) => (
              <Space>
                <a
                  onClick={() => {
                    setEditing(g);
                    form.setFieldsValue(g);
                    setOpen(true);
                  }}
                >
                  Изм.
                </a>
                <a onClick={() => del(g)} style={{ color: 'red' }}>
                  Удалить
                </a>
              </Space>
            ),
          },
        ]}
      />
      <Modal
        title={editing ? 'Редактировать группу' : 'Создать группу'}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={() => form.submit()}
      >
        <Form form={form} layout="vertical" onFinish={(v) => save.mutate(v)}>
          <Form.Item name="name" label="Название" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
        </Form>
      </Modal>
      <Drawer
        title={`Студенты группы «${studentsOf?.name}»`}
        open={!!studentsOf}
        onClose={() => setStudentsOf(null)}
        width={520}
      >
        <Table
          size="small"
          rowKey="id"
          dataSource={studentItems}
          pagination={false}
          columns={[
            { title: 'ФИО', dataIndex: 'full_name' },
            { title: 'Логин', dataIndex: 'login' },
            { title: 'Долг', render: (_, u) => fmtMinutes(u.debt_minutes) },
            {
              title: 'Активен',
              render: (_, u) => (u.is_active ? <Tag color="green">Да</Tag> : <Tag color="red">Нет</Tag>),
            },
          ]}
        />
      </Drawer>
    </>
  );
}
