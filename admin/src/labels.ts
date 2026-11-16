export const roleLabels: Record<string, string> = {
  student: 'Студент',
  teacher: 'Преподаватель',
  komendant: 'Комендант',
  admin: 'Администратор',
};

export const sessionStatusLabels: Record<string, { label: string; color: string }> = {
  active: { label: 'Активна', color: 'processing' },
  pending_confirmation: { label: 'Ждёт подтверждения', color: 'gold' },
  credited: { label: 'Зачтена', color: 'green' },
  komendant_rejected: { label: 'Отклонена комендантом', color: 'red' },
  expired: { label: 'Просрочена', color: 'volcano' },
};

export const photoReviewLabels: Record<string, { label: string; color: string }> = {
  pending: { label: 'На проверке', color: 'gold' },
  approved: { label: 'Зачтено', color: 'green' },
  rejected: { label: 'Не зачтено', color: 'red' },
};

export const difficultyLabels: Record<string, string> = {
  light: 'Лёгкая ×1',
  hard: 'Сложная ×2',
};

export const orderStatusLabels: Record<string, { label: string; color: string }> = {
  pending: { label: 'Ожидает выдачи', color: 'gold' },
  issued: { label: 'Выдан', color: 'green' },
  cancelled: { label: 'Отменён', color: 'red' },
};

export const actionLabels: Record<string, string> = {
  login: 'Вход',
  login_failed: 'Неудачный вход',
  qr_opened: 'QR открыт',
  session_started: 'Отработка начата',
  session_finished: 'Отработка завершена',
  session_confirmed: 'Отработка подтверждена',
  session_rejected: 'Отработка отклонена',
  session_expired: 'Отработка просрочена',
  photo_reviewed: 'Проверка фото',
  photo_viewed: 'Просмотр фото',
  assignment_created: 'Назначение часов',
  reward_created: 'Товар создан',
  reward_updated: 'Товар изменён',
  reward_deleted: 'Товар удалён',
  order_created: 'Заказ создан',
  order_issued: 'Заказ выдан',
  order_cancelled: 'Заказ отменён',
  user_created: 'Пользователь создан',
  user_updated: 'Пользователь изменён',
  user_deleted: 'Пользователь удалён',
  password_reset: 'Сброс пароля',
  password_changed: 'Смена пароля',
  adjusted: 'Корректировка долга/баланса',
  import_users: 'Импорт пользователей',
  summary_generated: 'Сводка сформирована',
  group_created: 'Группа создана',
  group_updated: 'Группа изменена',
  group_deleted: 'Группа удалена',
  push_token: 'Токен push',
};

export const actionLabel = (a: string) => actionLabels[a] ?? a;
