import { useEffect, useState } from 'react';
import { Image, Platform, type ImageProps, type ImageStyle, type StyleProp } from 'react-native';
import { getBaseUrl, getToken } from '../api/client';

/** <Image> for API paths that need the bearer token (session photos, reward images). */
export function AuthImage({
  path,
  style,
  ...props
}: { path: string; style?: StyleProp<ImageStyle> } & Omit<ImageProps, 'source' | 'style'>) {
  const token = getToken();
  const uri = `${getBaseUrl()}${path}`;
  const webUri = useWebBlobUrl(uri, token);
  return (
    <Image
      source={
        Platform.OS === 'web'
          ? { uri: webUri }
          : { uri, headers: token ? { Authorization: `Bearer ${token}` } : undefined }
      }
      style={style}
      {...props}
    />
  );
}

// react-native-web's <Image> ignores `headers`: fetch with the token and show a blob: URL
function useWebBlobUrl(uri: string, token: string | null) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    let alive = true;
    let created: string | undefined;
    fetch(uri, { headers: token ? { Authorization: `Bearer ${token}` } : {} })
      .then((r) => (r.ok ? r.blob() : Promise.reject(new Error(String(r.status)))))
      .then((b) => {
        if (!alive) return;
        created = URL.createObjectURL(b);
        setUrl(created);
      })
      .catch(() => {});
    return () => {
      alive = false;
      if (created) URL.revokeObjectURL(created);
    };
  }, [uri, token]);
  return url;
}
