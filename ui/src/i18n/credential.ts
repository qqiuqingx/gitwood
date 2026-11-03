import { detectLocale } from './index';
const messages = {
  en: {
    title: 'Git Credentials', username: 'Username', password: 'Password', passphrase: 'Passphrase',
    generic: 'Value', hostKey: 'Host Key',
    hostKeyHint: 'The host key is not known yet. Continue only if the fingerprint matches the one published by the server.',
    stored: 'Gitwood does not store credentials. Whether they are remembered depends on your Git credential helper.',
    continueConnection: 'Continue Connecting', confirm: 'OK', cancel: 'Cancel',
  },
  'zh-CN': {
    title: 'Git 凭据', username: '用户名', password: '密码', passphrase: '口令',
    generic: '输入值', hostKey: '主机密钥',
    hostKeyHint: '该主机密钥尚未记录。只有确认指纹与服务器公布的一致时才继续连接。',
    stored: 'Gitwood 不保存凭据。是否记住由你的 Git 凭据助手决定。',
    continueConnection: '继续连接', confirm: '确定', cancel: '取消',
  },
};
export type CredentialMessage = keyof typeof messages.en;
export function credentialText(key: CredentialMessage): string {
  return messages[detectLocale(navigator.language)][key];
}
