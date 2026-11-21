import { Button, Drawer, Form, Input, Modal, Space, Table, Tag, Typography, message } from 'antd';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../api/client';
import { fmtMinutes, fmtTs } from '../format';
import { UserSelect } from '../components/UserSelect';
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
  const [addId, setAddId] = useState<string>();
  const [newForm] = Form.useForm();
  const { data: students } = useQuery({
    queryKey: ['group-students', studentsOf?.id],
    enabled: !!studentsOf,
    queryFn: () => api.get<Paginated<UserDto> | UserDto[]>(`/api/groups/${studentsOf!.id}/students`),
  });
  const studentItems: UserDto[] = Array.isArray(students) ? students : (students?.items ?? []);
  const refreshGroup = () => {
    qc.invalidateQueries({ queryKey: ['groups'] });
    qc.invalidateQueries({ queryKey: ['group-students', studentsOf?.id] });
    qc.invalidateQueries({ queryKey: ['users'] });
  };
  // move an existing student into this group (or out of it with null)
  const setGroup = useMutation({
    mutationFn: (v: { userId: string; groupId: string | null }) =>
      api.patch(`/api/admin/users/${v.userId}`, { group_id: v.groupId }),
    onSuccess: () => {
      setAddId(undefined);
      refreshGroup();
    },
    onError: (e) => msg.error((e as Error).message),
  });
  const createStudent = useMutation({
    mutationFn: (v: { full_name: string; login: string }) =>
      api.post<{ user: UserDto; generated_password: string | null }>('/api/admin/users', {
        role: 'student',
        full_name: v.full_name,
        login: v.login,
        password: null,
        group_id: studentsOf!.id,
      }),
    onSuccess: (r) => {
      newForm.resetFields();
      refreshGroup();
      Modal.info({
        title: 'Студент создан',
        content: (
          <>
            Логин: <Typography.Text code>{r.user.login}</Typography.Text>
            <br />
            Пароль: <Typography.Text copyable code>{r.generated_password}</Typography.Text>
          </>
        ),
      });
    },
    onError: (e) => msg.error((e as Error).message),
  });

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
            render: (_, g) => Number(g.student_count ?? 0),
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
        confirmLoading={save.isPending}
        forceRender
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
        width={640}
      >
        <Space wrap style={{ marginBottom: 12 }}>
          <UserSelect role="student" value={addId} onChange={setAddId} placeholder="Добавить студента" />
          <Button
            disabled={!addId}
            loading={setGroup.isPending}
            onClick={() => addId && setGroup.mutate({ userId: addId, groupId: studentsOf!.id })}
          >
            Добавить в группу
          </Button>
        </Space>
        <Form
          form={newForm}
          layout="inline"
          onFinish={(v) => createStudent.mutate(v)}
          style={{ marginBottom: 12 }}
        >
          <Form.Item name="full_name" rules={[{ required: true, message: 'ФИО' }]}>
            <Input placeholder="ФИО нового студента" style={{ width: 220 }} />
          </Form.Item>
          <Form.Item name="login" rules={[{ required: true, message: 'Логин' }]}>
            <Input placeholder="Логин" style={{ width: 140 }} />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={createStudent.isPending}>
            Создать
          </Button>
        </Form>
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
            {
              title: '',
              render: (_, u) => (
                <a onClick={() => setGroup.mutate({ userId: u.id, groupId: null })}>Убрать</a>
              ),
            },
          ]}
        />
      </Drawer>
    </>
  );
}
