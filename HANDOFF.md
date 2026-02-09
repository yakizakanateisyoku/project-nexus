# Project Nexus — 引き継ぎ資料

> **最終更新**: 2026-02-10 (commit 7185c65)
> **リポジトリ**: https://github.com/yakizakanateisyoku/project-nexus
> **作業PC**: OMEN（Commander）

## プロジェクト概要

Tauri v2 + Rust + vanilla JS によるデスクトップAIハブアプリ。
OMEN を司令塔（Commander）として、LattePanda Sigma / Dell Precision を
SSH経由でリモート制御し、複数PCでAIタスクを分散実行する構想。

**設計原則**: 軽量・低コスト・外部crate最小限

---

## 進捗サマリー

| Phase | 内容 | 状態 |
|-------|------|------|
| 1 | 環境構築・基本UI・システムトレイ | ✅ 完了 |
| 2 | Anthropic API直接統合・会話管理 | ✅ 完了 |
| 3-A | SSH基盤構築（OMEN側） | ✅ 完了 |
| 3-C | UI拡張（マシン監視・リモート実行） | ✅ 完了 |
| 3-B | リモートPC環境セットアップ | ⬜ 未着手（物理アクセス必要） |
| 4 | スマート機能（ストリーミング等） | ⬜ 予定 |
| 5 | マルチAI連携（GPT-4o, Gemini） | ⬜ 予定 |
| 6 | 磨き込み（Notion連携, UI/UX） | ⬜ 予定 |

---

## Phase 1 ✅ 環境構築・基本UI

- Tauri v2 プロジェクトスキャフォールド (`npm create tauri-app`)
- ダークテーマ チャットUI（サイドバー + メインエリア）
- システムトレイ常駐（PNG アイコン、tray-icon feature）
- `start-nexus.bat` — 環境変数付き起動スクリプト

## Phase 2 ✅ Anthropic API直接統合

- **CLI廃止 → HTTP直接通信**: `reqwest` + `rustls-tls` でAPI呼出
- **会話履歴管理**: メモリ内 messages 配列、UIスレッド切替対応
- **モデル切替**: Haiku 4.5 / Sonnet 4.5 / Opus 4.5 セレクタ
- **APIキー管理**: `.env` → `dotenvy` でロード（GUI起動時のPATH問題対応済み）
- **トークン追跡**: 入出力トークン数リアルタイム表示
- **コンテキスト監視**: モデル別上限に対する使用率バー表示
- **コスト概算**: モデル別料金テーブルで累計コスト表示

## Phase 3-A ✅ SSH基盤構築（OMEN側）

- **SSH接続テスト** (`ssh_check_alive`): `ssh.exe` + tokio非同期、5秒タイムアウト
- **リモートコマンド実行** (`execute_remote_command`): 30秒タイムアウト
- **SSH設定管理**: `SshState`（Mutex）、CRUD操作
- **マシン定義**: OMEN=Commander(常時online), SIGMA/Precision=Remote
- **軽量設計**: 外部SSH crate不要、Windows標準 `ssh.exe` を利用
- tokio features: `sync, process, time, rt-multi-thread, macros`

## Phase 3-C ✅ UI拡張

- **マシンステータス**: 15秒ポーリングで全マシンSSH接続チェック
- **動的マシンリスト**: `renderMachineList()` でステータス反映
- **マシン選択**: Remoteマシンクリックで選択/解除
- **リモートコマンドパネル**: 選択マシンへのコマンド入力・実行・結果表示
- **ビジュアル**: パルスアニメーション(checking)、stdout緑/stderr赤

## Phase 3-B ⬜ リモートPC環境セットアップ（物理アクセス必要）

- ⬜ SIGMA: OpenSSH Server有効化、公開鍵配置
- ⬜ SIGMA: Node.js, Claude Code インストール
- ⬜ Precision: OpenSSH Server有効化、公開鍵配置
- ⬜ Precision: Node.js, Claude Code インストール
- ⬜ OMEN → SIGMA/Precision SSH鍵認証テスト

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

---

## Phase 4（予定）: スマート機能
- ⬜ ストリーミング応答（リアルタイム文字表示）— tokio導入済みで実装可能
- ⬜ 自動スレッド切替（コンテキスト満杯時）
- ⬜ 作業中断チェック＆自動引き継ぎ生成

## Phase 5（予定）: マルチAI連携
- ⬜ GPT-4o, Gemini Flash 統合
- ⬜ AI別タブ・比較表示

## Phase 6（予定）: 磨き込み
- ⬜ Notion連携
- ⬜ UI/UXブラッシュアップ
- ⬜ Markdown/コードハイライト表示

---

## ファイル構成

```
project-nexus/
├── HANDOFF.md              ← この引き継ぎ資料
├── README.md
└── nexus-app/
    ├── start-nexus.bat     # 環境変数付き起動 (.envからAPIキー読込)
    ├── package.json
    ├── src/
    │   ├── index.html      # チャットUI (サイドバー, リモートパネル)
    │   ├── styles.css      # ダークテーマ全スタイル
    │   └── main.js         # フロントエンド (会話, トークン, マシン監視, リモート実行)
    └── src-tauri/
        ├── Cargo.toml      # reqwest, tokio, dotenvy, tray-icon
        ├── tauri.conf.json
        └── src/
            ├── main.rs     # エントリポイント
            └── lib.rs      # Rustバックエンド全体
                            #   - Anthropic API呼出 (send_message)
                            #   - トークン追跡 (TokenTracker)
                            #   - SSH基盤 (SshState, ssh_check_alive, execute_remote_command)
                            #   - マシン管理 (get_machine_status, get/update_ssh_config)
                            #   - システムトレイ
```

## 技術メモ

- **Rust crate**: tauri 2, reqwest 0.12 (rustls-tls), tokio 1, serde, dotenvy, base64
- **SSH方式**: Windows標準 `ssh.exe` — 外部crateなし、将来 `russh` 移行可能
- **ポーリング**: 15秒間隔（SSH接続テストのオーバーヘッド最小化）
- **APIキー**: `.env` ファイル → `dotenvy` → `ANTHROPIC_API_KEY`
- **PATH競合注意**: Claude Desktop (`claude.exe`) と Claude Code CLI (`claude.cmd`) が共存
- **コスト意識**: Haiku 4.5デフォルト候補（入力$0.80/M vs Sonnet $3.0/M）

## コミット履歴

| Hash | 内容 |
|------|------|
| `7185c65` | Phase 3-A/3-C: SSH基盤構築 + UI拡張 |
| `cf8721f` | Fix: dotenvy for API key loading in GUI context |
| `a428a4f` | Phase3 Context monitoring and cost estimation |
| `8598503` | Phase 2: CLI to Anthropic API Direct Integration |
| `378a601` | Phase 1 tray + Phase 2 CLI integration |
| `e806397` | Phase 1 chat UI implementation |
| `06e5018` | Fix: package dependencies |
| `4261ae8` | Initial commit: Tauri project scaffold |
