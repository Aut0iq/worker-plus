import { Image, type ImageProps, type ImageStyle, type StyleProp } from 'react-native';
import { getBaseUrl, getToken } from '../api/client';

/** <Image> for API paths that need the bearer token (session photos, reward images). */
export function AuthImage({
  path,
  style,
  ...props
}: { path: string; style?: StyleProp<ImageStyle> } & Omit<ImageProps, 'source' | 'style'>) {
  const token = getToken();
  return (
    <Image
      source={{
        uri: `${getBaseUrl()}${path}`,
        headers: token ? { Authorization: `Bearer ${token}` } : undefined,
      }}
      style={style}
      {...props}
    />
  );
}
