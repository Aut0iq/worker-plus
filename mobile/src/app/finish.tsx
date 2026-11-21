import { useRef, useState } from 'react';
import { ActivityIndicator, Alert, Image, Platform, StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { useQueryClient } from '@tanstack/react-query';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { ImageManipulator, SaveFormat } from 'expo-image-manipulator';
import { api, ApiError, errorText } from '@/api/client';
import type { ActiveSessionResponse } from '@/api/types/ActiveSessionResponse';
import type { SessionDto } from '@/api/types/SessionDto';
import { Scanner, WEB_NO_CAMERA } from '@/components/Scanner';
import { Button, Card, ErrorText } from '@/components/ui';
import { makeStyles } from '@/theme';

type Step = 'photo' | 'sending' | 'done';

export default function FinishScreen() {
  return Platform.OS === 'web' ? <WebNoCamera /> : <NativeFinish />;
}

function WebNoCamera() {
  const styles = useStyles();
  return (
    <View style={styles.center}>
      <Card>
        <Text style={styles.sub}>{WEB_NO_CAMERA}</Text>
        <Button title="Назад" onPress={() => router.back()} style={{ marginTop: 16 }} />
      </Card>
    </View>
  );
}

// finishing = a photo of the work, then the komendant's finish QR (changes every 3 min);
// the QR is scanned right before sending so it cannot go stale while the student shoots
function NativeFinish() {
  const styles = useStyles();
  const qc = useQueryClient();
  const cam = useRef<CameraView>(null);
  const [perm, requestPerm] = useCameraPermissions();
  const [step, setStep] = useState<Step>('photo');
  const [scanning, setScanning] = useState(false);
  const [photo, setPhoto] = useState<{ uri: string } | null>(null);
  const [err, setErr] = useState<string | null>(null);
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
      // React Native uploads only file:// or content:// URIs; a bare path fails as "network error"
      const uri = /^(file|content):\/\//.test(res.uri) ? res.uri : `file://${res.uri}`;
      setPhoto({ uri });
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Не удалось сделать фото');
    }
  };

  const finished = (s: SessionDto) => {
    void qc.invalidateQueries({ queryKey: ['active-session'] });
    void qc.invalidateQueries({ queryKey: ['me'] });
    if (s.status === 'expired') {
      Alert.alert('Не засчитано', 'Общественно полезный труд длился больше лимита и не засчитан', [
        { text: 'OK', onPress: () => router.back() },
      ]);
    } else {
      setDone(s);
      setStep('done');
    }
  };

  const submit = async (qr: string) => {
    if (!photo) return;
    setStep('sending');
    setErr(null);
    try {
      const fd = new FormData();
      fd.append('qr', qr);
      fd.append('photo', {
        uri: photo.uri,
        name: 'photo.jpg',
        type: 'image/jpeg',
      } as unknown as Blob);
      finished(await api.post<SessionDto>('/api/student/sessions/finish', fd));
    } catch (e) {
      // the upload may have reached the server even though the answer was lost
      if (e instanceof ApiError && (e.status === 0 || e.code === 'no_active_session')) {
        try {
          const a = await api.get<ActiveSessionResponse>('/api/student/sessions/active');
          if (a.session?.status === 'pending_confirmation') return finished(a.session);
        } catch {
          // still offline: show the original error
        }
      }
      // the photo is kept: a QR error (expired, wrong kind) needs only a rescan
      setErr(errorText(e, 'Ошибка отправки'));
      setStep('photo');
    }
  };

  if (step === 'done' && done) {
    return (
      <View style={styles.center}>
        <Card>
          <Text style={styles.title}>Фото отправлено</Text>
          <Text style={styles.sub}>Общественно полезный труд ожидает подтверждения коменданта.</Text>
          <Button title="На главную" onPress={() => router.back()} style={{ marginTop: 16 }} />
        </Card>
      </View>
    );
  }

  return (
    <View style={styles.wrap}>
      <Scanner
        visible={scanning}
        onClose={() => setScanning(false)}
        onScan={(data) => {
          setScanning(false);
          void submit(data);
        }}
      />
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
                  title="Сканировать QR и отправить"
                  onPress={() => {
                    setErr(null);
                    setScanning(true);
                  }}
                  style={{ flex: 1.4, marginLeft: 8 }}
                />
              </View>
            </View>
          )}
        </>
      )}
      {step === 'sending' && (
        <View style={styles.sending}>
          <ActivityIndicator size="large" color="#fff" />
          <Text style={{ color: '#fff', marginTop: 12 }}>Отправка…</Text>
        </View>
      )}
      {!!err && (
        <View style={styles.errBox}>
          <ErrorText>{err}</ErrorText>
        </View>
      )}
    </View>
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
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
  sending: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: '#000',
  },
  errBox: { backgroundColor: colors.card, padding: 12 },
  title: { fontSize: 20, fontWeight: '700', color: colors.text, textAlign: 'center' },
  sub: { fontSize: 14, color: colors.muted, textAlign: 'center', marginTop: 8 },
}));
