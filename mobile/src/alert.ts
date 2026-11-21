import { Alert, Platform, type AlertButton } from 'react-native';

// react-native-web's Alert.alert is a no-op: in the browser use window.confirm / window.alert
export function showAlert(title: string, message?: string, buttons?: AlertButton[]) {
  if (Platform.OS !== 'web') return Alert.alert(title, message, buttons);
  const text = message ? `${title}\n\n${message}` : title;
  const action = buttons?.find((b) => b.style !== 'cancel');
  if (buttons && buttons.length > 1) {
    if (window.confirm(text)) action?.onPress?.();
  } else {
    window.alert(text);
    action?.onPress?.();
  }
}
