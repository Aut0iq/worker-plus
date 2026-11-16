# NEEDTOFIX — известные проблемы и доработки

## Push-уведомления не приходят на телефон (релизная сборка)

**Симптом:** на телефоне нет системного push-уведомления, когда преподаватель
назначает часы, комендант подтверждает сессию и т.д.

**Что проверено (2026-11-16):** внутриприложенная лента уведомлений работает —
запись `kind=assignment` создаётся и доставляется (`GET /api/me/notifications`
вернул «Назначены часы / Вам назначено 120 мин», `read_at` проставлен —
приложение её получило и прочитало). Не работает именно Expo Push.

**Причина:** push-токен регистрируется только в dev/EAS-сборке с
`extra.eas.projectId`. В релизном APK (GitHub Actions `release-android.yml`)
projectId не задан → `registerForPushNotificationsAsync` не вызывается →
`POST /api/me/push-token` не делается → у сервера нет push-токена устройства,
`push_later` уходит в никуда. Дополнительно на сервере должен быть
`EXPO_PUSH_ENABLED` (проверить, что не выключен в `.env` тестового стенда).

**Что сделать:**
1. Создать проект на expo.dev, получить `projectId` (EAS).
2. Прописать `extra.eas.projectId` в `mobile/app.json`.
3. В `release-android.yml` добавить шаг: `eas init`/запись projectId перед
   prebuild (или захардкодить в `app.json` и закоммитить).
4. Убедиться, что в `.env` сервера `EXPO_PUSH_ENABLED=true`.
5. Пересобрать APK тегом `v1.0.1`, проверить: вход → токен в
   `POST /api/me/push-token` → push при назначении часов.

**Детали:** `mobile/src/screens/NotificationsScreen.tsx` и регистрация токена
(см. `push-token` в `me.rs`, маршруты `POST/DELETE /api/me/push-token`).

## Остальное из HANDOFF (для полноты)

- 9 moderate `npm audit` в админке — нужен переход react-router 6→7 (breaking).
- Дефолтные иконки/splash Expo — заменить на фирменные.
- Реальный проход QR-сценария двумя устройствами ещё не сделан.
