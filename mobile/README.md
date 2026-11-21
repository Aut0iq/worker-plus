# Work+ — мобильное приложение

Expo (React Native) приложение для студентов, комендантов и преподавателей
(админ входит в интерфейс преподавателя и дополнительно может списывать часы).
Package id — `ru.workerplus.app`.

## Запуск

```bash
npm install
npx expo start        # затем отсканировать QR в Expo Go (Android/iOS)
```

Адрес API: переменная `EXPO_PUBLIC_API_URL` (по умолчанию `http://185.28.173.203`,
в веб-версии — адрес, с которого открыта страница), либо поле «Адрес сервера» на
экране входа (сохраняется в SecureStore, в вебе — в `localStorage`).

## Веб-версия

`npx expo export -p web` собирает SPA с базовым путём `/app` (`app.json`:
`web.output: "single"`, `experiments.baseUrl`). В Docker-образе её раздаёт сервер
по `/app`. Камера в браузере работает только по HTTPS, поэтому на вебе сканер QR и
фото работы заменены сообщением (`WEB_NO_CAMERA`); подтверждения — через
`window.confirm` (`src/alert.ts`), хранилище — `src/storage.ts`.

## Android APK и push

APK собирается только в GitHub Actions по тегу `v*` (см. корневой README).
Push работает только в APK (в Expo Go удалённые push не работают с SDK 53):
`mobile/app.config.ts` подставляет `EAS_PROJECT_ID` и `GOOGLE_SERVICES_FILE` из
окружения, ключ FCM V1 лежит в Expo, на сервере — `EXPO_PUSH_ENABLED=true`.

## Скрипты

- `npm run sync-types` — скопировать DTO-типы из `../shared/api-types`
- `npm run typecheck` — sync-types + `tsc --noEmit`
- `npm run lint` — eslint (expo)
- `npm test` — юнит-тесты `src/format.test.ts` (node:test)
