
#### 3-A: SSH基盤構築（物理アクセス不要・OMEN側準備）✅ 完了
- ✅ Rust SSH実装: `std::process::Command` + `ssh.exe` 軽量案採用
- ✅ tokio features拡張 (`sync`, `process`, `time`, `rt-multi-thread`, `macros`)
- ✅ SSH接続設定管理 (`SshState` — `~/.ssh/config` Host名ベース)
- ✅ `get_machine_status` → 実SSH ping (`ssh_check_alive`) に置換
- ✅ `execute_remote_command(machine, command)` Tauriコマンド追加
- ✅ `get_ssh_config` / `update_ssh_config` 設定管理コマンド追加
- ✅ タイムアウト制御 (接続テスト5秒 / コマンド実行30秒)

#### 3-B: リモートPC環境セットアップ（物理アクセス必要）
- ⬜ SIGMA: OpenSSH Server有効化、公開鍵配置
- ⬜ SIGMA: Node.js, Claude Code インストール
- ⬜ Precision: OpenSSH Server有効化、公開鍵配置
- ⬜ Precision: Node.js, Claude Code インストール
- ⬜ OMEN → SIGMA/Precision SSH鍵認証テスト

#### 3-C: UI拡張 ✅ 完了
- ✅ サイドバーPC接続ステータスをリアルタイム化（15秒ポーリング）
- ✅ マシンクリックで選択→リモートコマンドパネル表示
- ✅ リモートコマンド入力・実行・結果表示パネル
- ✅ SSH接続チェック中のパルスアニメーション

### 実装方針

**SSH軽量案（採用済み）:**
- `std::process::Command` + Windows `ssh.exe` → 外部SSH crate不要
- `~/.ssh/config` のHost名で接続管理
- tokio非同期 + タイムアウト制御（接続テスト5秒、コマンド実行30秒）
- 将来的に`russh` crateへの移行も可能（接続プール、より堅牢なエラーハンドリング）

### SSH接続情報（予定）
```
# ~/.ssh/config に追記
Host sigma
    HostName [SIGMA_IP]
    User [USER]
    IdentityFile ~/.ssh/nexus_key
    StrictHostKeyChecking no

Host precision
    HostName [PRECISION_IP]
    User [USER]
    IdentityFile ~/.ssh/nexus_key
    StrictHostKeyChecking no
```

## Phase 4（予定）: スマート機能
- ⬜ ストリーミング応答（リアルタイム文字表示）→ tokio導入後に実装
- ⬜ 自動スレッド切り替え（コンテキスト満杯時）
- ⬜ 作業中断チェック＆自動引き継ぎ生成

## Phase 5（予定）: マルチAI連携
- ⬜ GPT-4o, Gemini Flash 統合
- ⬜ AI別タブ・比較表示

## Phase 6（予定）: 磨き込み
- ⬜ Notion連携
- ⬜ UI/UXブラッシュアップ
- ⬜ Markdown/コードハイライト表示

## 主要ファイル構成
```
nexus-app/
├── src/
│   ├── index.html    # チャットUI構造
│   ├── styles.css    # ダークテーマCSS
│   └── main.js       # チャットロジック (token tracking, context warning, machine polling, remote exec)
├── src-tauri/
│   ├── src/lib.rs    # Rustバックエンド (API呼出, トークン追跡, SSH基盤, tray)
│   ├── src/main.rs   # エントリポイント
│   ├── Cargo.toml    # reqwest, tokio(process/time), rustls-tls, dotenvy, tray-icon
│   └── tauri.conf.json
├── start-nexus.bat   # 環境変数付き起動スクリプト
└── package.json
```

## 設計メモ
- **Phase 3-Aでtokio導入済み** (`process`, `time`, `rt-multi-thread`, `macros`)
- **SSH軽量案採用**: `ssh.exe` + `tokio::process::Command`（外部SSH crateなし）
- **PATH競合**: Windowsでは Claude Desktop (claude.exe) と Claude Code CLI (claude.cmd) が共存
- **コスト意識**: Haiku 4.5をデフォルト候補に検討（入力$0.80/M vs Sonnet $3.0/M）
- **ポーリング間隔**: 15秒（SSH接続テストのオーバーヘッド最小化、必要に応じて調整可能）
