import { Button, Card, Form, Input, message } from 'antd';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../auth';
import { ApiError } from '../api/client';

export default function LoginPage() {
  const { login } = useAuth();
  const nav = useNavigate();
  const [msg, ctx] = message.useMessage();
  const onFinish = async (v: { login: string; password: string }) => {
    try {
      await login(v.login, v.password);
      nav('/', { replace: true });
    } catch (e) {
      msg.error(e instanceof ApiError ? e.message : (e as Error).message);
    }
  };
  return (
    <div style={{ minHeight: '100vh', display: 'grid', placeItems: 'center', background: '#f0f2f5' }}>
      {ctx}
      <Card title="Work+ — вход для администратора" style={{ width: 360 }}>
        <Form layout="vertical" onFinish={onFinish}>
          <Form.Item name="login" label="Логин" rules={[{ required: true }]}>
            <Input autoFocus />
          </Form.Item>
          <Form.Item name="password" label="Пароль" rules={[{ required: true }]}>
            <Input.Password />
          </Form.Item>
          <Button type="primary" htmlType="submit" block>
            Войти
          </Button>
        </Form>
      </Card>
    </div>
  );
}
