import { useRef, useState } from 'react';
import { Alert, Image, StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { useQueryClient } from '@tanstack/react-query';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { ImageManipulator, SaveFormat } from 'expo-image-manipulator';
import { api, ApiError } from '@/api/client';
import type { SessionDto } from '@/api/types/SessionDto';
import { Scanner } from '@/components/Scanner';
import { Button, Card, ErrorText } from '@/components/ui';
import { colors } from '@/theme';

type Step = 'photo' | 'scan' | 'done';

export default function FinishScreen() {
  const qc = useQueryClient();
  const cam = useRef<CameraView>(null);
  const [perm, requestPerm] = useCameraPermissions();
  const [step, setStep] = useState<Step>('photo');
  const [photo, setPhoto] = useState<{ uri: string } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<SessionDto | null>(null);

  const shoot = async () => {
    setErr(null);
    try {
      const pic = await cam.current?.takePictureAsync({ quality: 0.7 });
      if (!pic) return;
      const ctx = ImageManipulator.manipulate(pic.uri);
      ctx.resize({ width: 1600 });
      const out = await ctx.renderAsync();
      const res = await out.saveAsync({ compress: 0.7, format: SaveFormat.JPEG });
      setPhoto({ uri: res.uri });
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Не удалось сделать фото');
    }
  };

  const submit = async (qr: string) => {
    if (!photo) return;
    setBusy(true);
    setErr(null);
    try {
      const fd = new FormData();
      fd.append('qr', qr);
      fd.append('photo', {
        uri: photo.uri,
        name: 'photo.jpg',
        type: 'image/jpeg',
      } as unknown as Blob);
      const s = await api.post<SessionDto>('/api/student/sessions/finish', fd);
      void qc.invalidateQueries({ queryKey: ['active-session'] });
      void qc.invalidateQueries({ queryKey: ['me'] });
      if (s.status === 'expired') {
        Alert.alert('Не засчитано', 'Отработка длилась больше лимита и не засчитана', [
          { text: 'OK', onPress: () => router.back() },
        ]);
      } else {
        setDone(s);
        setStep('done');
      }
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : 'Ошибка отправки');
      setStep('photo');
    } finally {
      setBusy(false);
    }
  };

  if (step === 'done' && done) {
    return (
      <View style={styles.center}>
        <Card>
          <Text style={styles.title}>Фото отправлено</Text>
          <Text style={styles.sub}>Отработка ожидает подтверждения коменданта.</Text>
          <Button title="На главную" onPress={() => router.back()} style={{ marginTop: 16 }} />
        </Card>
      </View>
    );
  }

  return (
    <View style={styles.wrap}>
      {step === 'photo' && (
        <>
          {!photo ? (
            perm?.granted ? (
              <View style={{ flex: 1 }}>
                <CameraView ref={cam} style={{ flex: 1 }} />
                <View style={styles.bottom}>
                  <Button title="Сделать фото" onPress={() => void shoot()} />
                </View>
              </View>
            ) : (
              <View style={styles.center}>
                <Text style={styles.sub}>Нужен доступ к камере, чтобы сфотографировать работу.</Text>
                <Button
                  title="Разрешить камеру"
                  onPress={() => void requestPerm()}
                  style={{ marginTop: 12 }}
                />
              </View>
            )
          ) : (
            <View style={{ flex: 1 }}>
              <Image source={{ uri: photo.uri }} style={{ flex: 1 }} resizeMode="contain" />
              <View style={styles.bottomRow}>
                <Button title="Переснять" outline onPress={() => setPhoto(null)} style={{ flex: 1 }} />
                <Button
                  title="Далее — сканировать QR"
                  onPress={() => setStep('scan')}
                  style={{ flex: 1.4, marginLeft: 8 }}
                />
              </View>
            </View>
          )}
        </>
      )}
      {busy && (
        <View style={styles.overlay}>
          <Text style={{ color: '#fff' }}>Отправка…</Text>
        </View>
      )}
      <ErrorText>{err}</ErrorText>
      <Scanner
        visible={step === 'scan'}
        onClose={() => setStep('photo')}
        onScan={(qr) => void submit(qr)}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  wrap: { flex: 1, backgroundColor: '#000' },
  center: { flex: 1, justifyContent: 'center', padding: 24, backgroundColor: colors.bg },
  bottom: { position: 'absolute', bottom: 40, left: 24, right: 24 },
  bottomRow: {
    position: 'absolute',
    bottom: 40,
    left: 24,
    right: 24,
    flexDirection: 'row',
  },
  overlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: '#0008',
  },
  title: { fontSize: 20, fontWeight: '700', color: colors.text, textAlign: 'center' },
  sub: { fontSize: 14, color: colors.muted, textAlign: 'center', marginTop: 8 },
});
