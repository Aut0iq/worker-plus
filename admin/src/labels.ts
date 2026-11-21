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

export const auditActionLabels: Record<string, string> = {
  assignment_create: 'Назначение часов',
  group_create: 'Группа создана',
  group_update: 'Группа изменена',
  group_delete: 'Группа удалена',
  login_success: 'Вход',
  login_failure: 'Неудачный вход',
  login_blocked: 'Вход заблокирован',
  order_buy: 'Покупка награды',
  order_issue: 'Заказ выдан',
  order_cancel: 'Заказ отменён',
  password_change: 'Смена пароля',
  password_reset: 'Сброс пароля',
  photo_review: 'Проверка фото',
  qr_open: 'QR открыт',
  reward_create: 'Товар создан',
  reward_update: 'Товар изменён',
  reward_delete: 'Товар удалён',
  reward_image: 'Фото товара',
  session_start: 'ОПТ начат',
  session_finish: 'ОПТ завершён',
  session_confirm: 'ОПТ подтверждён',
  session_reject: 'ОПТ отклонён',
  session_expire: 'ОПТ просрочен',
  summary_generate: 'Сводка сформирована',
  user_create: 'Пользователь создан',
  user_update: 'Пользователь изменён',
  user_deactivate: 'Пользователь деактивирован',
  user_adjust: 'Корректировка задания/баланса',
  users_import: 'Импорт пользователей',
};

export const komendantActionLabels: Record<string, string> = {
  login: 'Вход',
  qr_open: 'QR открыт',
  photo_view: 'Просмотр фото',
  session_started: 'ОПТ начат',
  session_finished: 'ОПТ завершён',
  session_confirmed: 'ОПТ подтверждён',
  session_rejected: 'ОПТ отклонён',
  session_expired: 'ОПТ просрочен',
};

export const actionLabel = (a: string) => auditActionLabels[a] ?? komendantActionLabels[a] ?? a;
