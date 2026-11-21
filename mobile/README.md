# Prila — мобильное приложение

Expo (React Native) приложение для студентов, комендантов и преподавателей.

## Запуск

```bash
npm install
npx expo start        # затем отсканировать QR в Expo Go (Android/iOS)
```

Адрес API: переменная `EXPO_PUBLIC_API_URL` (по умолчанию `http://185.28.173.203`),
либо поле «Адрес сервера» на экране входа (сохраняется в SecureStore).

## Push-уведомления

В Expo Go удалённые push не работают (SDK 53+). Для push нужна development/EAS
сборка и `extra.eas.projectId` в app.json, а на сервере — `EXPO_PUSH_ENABLED=true`.

## Скрипты

- `npm run sync-types` — скопировать DTO-типы из `../shared/api-types`
- `npm run typecheck` — sync-types + `tsc --noEmit`
- `npm run lint` — eslint (expo)
- `npm test` — юнит-тесты `src/format.test.ts` (node:test)
