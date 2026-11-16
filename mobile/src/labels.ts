export const roleLabels: Record<string, string> = {
  student: 'Студент',
  teacher: 'Преподаватель',
  komendant: 'Комендант',
  admin: 'Администратор',
};

export const sessionStatusLabels: Record<string, { label: string; color: string }> = {
  active: { label: 'Активна', color: '#208AEF' },
  pending_confirmation: { label: 'Ждёт подтверждения', color: '#d48806' },
  credited: { label: 'Зачтена', color: '#389e0d' },
  komendant_rejected: { label: 'Отклонена комендантом', color: '#d4380d' },
  expired: { label: 'Просрочена', color: '#cf1322' },
};

export const photoReviewLabels: Record<string, { label: string; color: string }> = {
  pending: { label: 'На проверке', color: '#d48806' },
  approved: { label: 'Зачтено', color: '#389e0d' },
  rejected: { label: 'Не зачтено', color: '#d4380d' },
};

export const difficultyLabels: Record<string, string> = {
  light: 'Лёгкая ×1',
  hard: 'Сложная ×2',
};

export const orderStatusLabels: Record<string, { label: string; color: string }> = {
  pending: { label: 'Ожидает выдачи', color: '#d48806' },
  issued: { label: 'Выдан', color: '#389e0d' },
  cancelled: { label: 'Отменён', color: '#d4380d' },
};
