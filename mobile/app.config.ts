import type { ConfigContext, ExpoConfig } from 'expo/config';

// Extends app.json with values that must not be hard-coded in the public repo.
// Set them in the environment (locally, or as GitHub secrets in release-android.yml):
//   EAS_PROJECT_ID        - expo.dev project id, required for push tokens
//   GOOGLE_SERVICES_FILE  - path to google-services.json (Firebase), required for Android push
export default ({ config }: ConfigContext): ExpoConfig => {
  const projectId = process.env.EAS_PROJECT_ID;
  const googleServicesFile = process.env.GOOGLE_SERVICES_FILE;
  return {
    ...config,
    name: config.name ?? 'Work+',
    slug: config.slug ?? 'worker-plus',
    android: {
      ...config.android,
      ...(googleServicesFile ? { googleServicesFile } : {}),
    },
    extra: {
      ...config.extra,
      ...(projectId ? { eas: { ...config.extra?.eas, projectId } } : {}),
    },
  };
};
