import { useEffect, useRef } from 'react';
import { Modal, StyleSheet, Text, View } from 'react-native';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { Button } from './ui';
import { colors } from '../theme';

/** Full-screen QR scanner. Fires onScan once, then waits for onClose. */
export function Scanner({
  visible,
  onScan,
  onClose,
}: {
  visible: boolean;
  onScan: (data: string) => void;
  onClose: () => void;
}) {
  const [perm, requestPerm] = useCameraPermissions();
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

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: '#000' },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24 },
  msg: { color: '#fff', fontSize: 16, textAlign: 'center', marginBottom: 16 },
  bottom: { position: 'absolute', bottom: 40, left: 24, right: 24 },
  muted: { color: colors.muted },
});
