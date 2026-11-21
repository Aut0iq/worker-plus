import { useEffect, useRef } from 'react';
import { Modal, Platform, StyleSheet, Text, View } from 'react-native';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { Button } from './ui';
import { makeStyles } from '../theme';

// browsers give a page the camera only over HTTPS; the server is plain HTTP for now
export const WEB_NO_CAMERA =
  'В веб-версии камера пока недоступна. Сканировать QR и фотографировать работу можно в приложении для Android.';

type Props = { visible: boolean; onScan: (data: string) => void; onClose: () => void };

/** Full-screen QR scanner. Fires onScan once, then waits for onClose. */
export function Scanner(props: Props) {
  return Platform.OS === 'web' ? <WebNoScanner {...props} /> : <NativeScanner {...props} />;
}

function WebNoScanner({ visible, onClose }: Props) {
  const styles = useStyles();
  return (
    <Modal visible={visible} animationType="slide" onRequestClose={onClose}>
      <View style={styles.wrap}>
        <View style={styles.center}>
          <Text style={styles.msg}>{WEB_NO_CAMERA}</Text>
          <Button title="Закрыть" onPress={onClose} />
        </View>
      </View>
    </Modal>
  );
}

function NativeScanner({ visible, onScan, onClose }: Props) {
  const [perm, requestPerm] = useCameraPermissions();
  const styles = useStyles();
  const fired = useRef(false);

  useEffect(() => {
    if (visible && perm && !perm.granted && perm.canAskAgain) void requestPerm();
  }, [visible, perm, requestPerm]);

  const handle = ({ data }: { data: string }) => {
    if (fired.current) return;
    fired.current = true;
    onScan(data);
  };

  return (
    <Modal
      visible={visible}
      animationType="slide"
      onRequestClose={onClose}
      onShow={() => {
        fired.current = false;
      }}
    >
      <View style={styles.wrap}>
        {perm?.granted ? (
          <CameraView
            style={StyleSheet.absoluteFill}
            barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
            onBarcodeScanned={handle}
          />
        ) : (
          <View style={styles.center}>
            <Text style={styles.msg}>
              {perm && !perm.canAskAgain && !perm.granted
                ? 'Нет доступа к камере. Разрешите камеру в настройках.'
                : 'Запрашиваем доступ к камере…'}
            </Text>
            {perm && !perm.granted && perm.canAskAgain && (
              <Button title="Разрешить" onPress={() => void requestPerm()} />
            )}
          </View>
        )}
        <View style={styles.bottom}>
          <Button title="Отмена" outline onPress={onClose} style={{ backgroundColor: '#0008' }} />
        </View>
      </View>
    </Modal>
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  wrap: { flex: 1, backgroundColor: '#000' },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  msg: { color: '#fff', fontSize: 16, textAlign: 'center', marginBottom: 16 },
  bottom: { position: 'absolute', bottom: 40, left: 24, right: 24 },
  muted: { color: colors.muted },
}));
