import { detectLocale } from './index';
const messages = {
  en: {
    updateCompleted: 'Project updated', title: 'Merge Conflicts', resolve: 'Resolve Conflicts', yours: 'Accept Yours', theirs: 'Accept Theirs', merge: 'Merge…',
    local: 'Local (Yours)', incoming: 'Incoming (Theirs)', rebased: 'Local changes (Git theirs)', upstream: 'Upstream (Git ours)', result: 'Result',
    close: 'Close', cancel: 'Cancel', discard: 'Discard and Close', confirm: 'Unresolved changes or edits will be discarded. Close without applying?',
    save: 'Apply and Mark Resolved', allResolved: 'All conflicts are resolved. Continue the operation or close this dialog.', continue: 'Continue',
    auto: 'Non-conflicting changes applied automatically', left: 'Accept Left', right: 'Accept Right', both: 'Accept Both', ignore: 'Ignore',
    binaryOrLarge: 'Binary, unsupported encoding or large file: text merge is unavailable. Accept a side from the file list.',
    unsupported: 'Submodule or symbolic link: text merge is unavailable.', encoding: 'The working file encoding cannot be decoded safely.',
    error: 'Conflict resolution failed', loading: 'Loading…', block: 'Conflict', unresolved: 'unresolved',
    DD: 'Both deleted', AU: 'Added by us', UD: 'Deleted by them', UA: 'Added by them', DU: 'Deleted by us', AA: 'Both added', UU: 'Both modified',
    binary: 'Binary / non-text / large', submodule: 'Submodule', rebaseHint: 'During rebase Yours uses Git stage 3; Theirs uses stage 2.',
  },
  'zh-CN': {
    updateCompleted: '项目更新完成', title: '合并冲突', resolve: '解决冲突', yours: '接受本地', theirs: '接受传入', merge: '合并…',
    local: '本地', incoming: '传入', rebased: '本地改动（Git theirs）', upstream: '上游（Git ours）', result: '结果',
    close: '关闭', cancel: '取消', discard: '放弃并关闭', confirm: '未解决的更改及编辑结果将被放弃，是否不应用直接关闭？',
    save: '应用并标记已解决', allResolved: '所有冲突已解决。可以继续操作或关闭对话框。', continue: '继续',
    auto: '已自动应用非冲突改动', left: '接受左侧', right: '接受右侧', both: '接受两者', ignore: '忽略',
    binaryOrLarge: '二进制、未知编码或大文件无法进行文本合并。可以在文件列表中接受某一侧。',
    unsupported: '子模块或符号链接不支持文本合并。', encoding: '无法安全解码工作区文件的编码。',
    error: '解决冲突失败', loading: '加载中…', block: '冲突块', unresolved: '个未解决',
    DD: '两边删除', AU: '本地新增', UD: '传入删除', UA: '传入新增', DU: '本地删除', AA: '两边新增', UU: '两边修改',
    binary: '二进制／非文本／大文件', submodule: '子模块', rebaseHint: '变基时“本地”使用 Git stage 3，“传入”使用 stage 2。',
  },
};
export type ConflictMessage = keyof typeof messages.en;
export const conflictText = (key: ConflictMessage) => messages[detectLocale(navigator.language)][key];
