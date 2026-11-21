import { useState } from 'react';
import { KeyboardAvoidingView, Platform, StyleSheet, Text, View } from 'react-native';
import { router } from 'expo-router';
import { ApiError, errorText, getBaseUrl, saveServerUrl, setBaseUrl } from '@/api/client';
import { useAuth } from '@/auth';
import { Button, ErrorText, Field } from '@/components/ui';
import { makeStyles } from '@/theme';

export default function LoginScreen() {
  const styles = useStyles();
  const { login } = useAuth();
  const [loginName, setLoginName] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [showServer, setShowServer] = useState(false);
  const [serverUrl, setServerUrl] = useState(getBaseUrl());

  const submit = async () => {
    setErr(null);
    setBusy(true);
    try {
      await login(loginName.trim(), password);
      router.replace('/');
    } catch (e) {
      setErr(e instanceof ApiError && e.status === 401 ? e.message : errorText(e, 'Ошибка входа'));
    } finally {
      setBusy(false);
    }
  };

  const saveServer = async () => {
    const u = serverUrl.trim();
    setBaseUrl(u);
    await saveServerUrl(u || null);
    setShowServer(false);
  };

  return (
    <KeyboardAvoidingView
      style={styles.wrap}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <View style={styles.box}>
        <Text style={styles.title}>Work+</Text>
        <Text style={styles.sub}>Учёт часов отработок</Text>
        <Field
          label="Логин"
          value={loginName}
          onChangeText={setLoginName}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <Field
          label="Пароль"
          value={password}
          onChangeText={setPassword}
          secureTextEntry
          autoCapitalize="none"
          onSubmitEditing={() => void submit()}
        />
        <ErrorText>{err}</ErrorText>
        <Button title="Войти" loading={busy} onPress={() => void submit()} style={{ marginTop: 12 }} />
        <Text style={styles.serverToggle} onPress={() => setShowServer((v) => !v)}>
          Адрес сервера
        </Text>
        {showServer && (
          <View>
            <Field
              label="URL сервера"
              value={serverUrl}
              onChangeText={setServerUrl}
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="url"
            />
            <Button title="Сохранить" outline onPress={() => void saveServer()} />
          </View>
        )}
      </View>
    </KeyboardAvoidingView>
  );
}

const useStyles = makeStyles((colors) =>
  StyleSheet.create({
  wrap: { flex: 1, backgroundColor: colors.bg, justifyContent: 'center' },
  box: { marginHorizontal: 24, backgroundColor: colors.card, borderRadius: 16, padding: 20 },
  title: { fontSize: 30, fontWeight: '800', color: colors.primary, textAlign: 'center' },
  sub: { fontSize: 14, color: colors.muted, textAlign: 'center', marginBottom: 20, marginTop: 4 },
  serverToggle: {
    color: colors.muted,
    textAlign: 'center',
    marginTop: 16,
    marginBottom: 8,
    textDecorationLine: 'underline',
  },
}));
