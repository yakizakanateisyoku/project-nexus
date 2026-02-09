# 🔗 Project Nexus

**マルチPC × マルチAI オーケストレーションハブ**
**Multi-PC × Multi-AI Orchestration Hub**

OMENを司令塔として、全PC・全AIを一つのデスクトップウィジェットから統括操作するシステム。
Control all your PCs and AIs from a single desktop widget — no remote desktop needed.

---

## 📌 概要 / Overview

| 項目 / Item | 内容 / Detail |
|---|---|
| プロジェクト名 | Project Nexus |
| 目的 / Purpose | リモートデスクトップ不要で全PCにタスク指示・ステータス監視・マルチAI活用 |
| UIフレームワーク | Tauri 2.x (Rust + HTML/CSS/JS) |
| 司令塔 / Commander | OMEN |
| リモートPC / Remote | SIGMA, Precision (拡張可能) |
| AI連携 / AI Integration | Claude Code (メイン), GPT-4o, Gemini Flash |

---

## 🏗 アーキテクチャ / Architecture

```
┌─ Nexus (Tauri App on OMEN) ──────────────────────┐
│                                                    │
│  UI Layer (HTML/CSS/JS)                            │
│  ├─ チャットパネル (プロジェクト別)                  │
│  ├─ ステータスモニター (各PC)                       │
│  └─ ログビューア                                   │
│                                                    │
│  Backend (Rust)                                    │
│  ├─ SSH Manager → SIGMA / Precision               │
│  ├─ Claude Code CLI Interface                     │
│  ├─ Multi-AI Gateway (GPT-4o, Gemini Flash)       │
│  ├─ Context Manager (トークン監視)                 │
│  └─ Status Poller (status.json 定期取得)          │
│                                                    │
│  System Tray 常駐                                  │
└────────────────────────────────────────────────────┘
         │SSH              │SSH
         ▼                 ▼
      SIGMA             Precision
    Claude Code         Claude Code
    status.json         status.json
```

リモートPCには常駐プロセス不要。SSH経由でオンデマンド操作。

---

## 🚀 セットアップ / Setup

### 前提条件 / Prerequisites

| ツール | バージョン | 用途 |
|---|---|---|
| [Node.js](https://nodejs.org/) | v20 LTS+ | Claude Code CLI, npm |
| [Rust](https://rustup.rs/) | 1.70+ | Tauri バックエンド (OMENのみ) |
| [VS Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) | C++ コンポーネント | Rust コンパイル (Windowsのみ) |
| [Git](https://git-scm.com/) | 最新 | バージョン管理 |

### OMEN（司令塔）セットアップ

```bash
# 1. リポジトリクローン
git clone https://github.com/yakizakanateisyoku/project-nexus.git
cd project-nexus/nexus-app

# 2. 依存インストール
npm install

# 3. 開発サーバー起動
npm run tauri dev

# 4. ビルド（リリース用）
npm run tauri build
```

### リモートPC（SIGMA / Precision）セットアップ

リモートPCは軽量構成。Tauri/Rustは不要。

```powershell
# 1. OpenSSH Server 有効化（Windows 設定 → オプション機能）
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0

# 2. SSH サービス起動＆自動起動
Start-Service sshd
Set-Service sshd -StartupType Automatic

# 3. Claude Code インストール
npm install -g @anthropic-ai/claude-code

# 4. Claude Code 認証
claude
# → ブラウザでログイン

# 5. SSH 鍵認証設定（OMEN から接続するため）
# OMEN 側で生成した公開鍵を authorized_keys に追加
```

---

## 📁 プロジェクト構成 / Project Structure

```
project-nexus/
├── README.md              ← このファイル
├── .gitignore
└── nexus-app/             ← Tauri アプリ本体
    ├── package.json       ← npm 設定・スクリプト
    ├── src/               ← フロントエンド (HTML/JS/CSS)
    │   ├── index.html
    │   ├── main.js
    │   └── styles.css
    ├── src-tauri/         ← Rust バックエンド
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs
    │   │   └── main.rs
    │   ├── tauri.conf.json
    │   ├── capabilities/
    │   └── icons/
    └── node_modules/
```

---

## 📅 ロードマップ / Roadmap

| Phase | 内容 | 状態 |
|---|---|---|
| **Phase 1** | 基盤構築 — Tauri scaffold, 環境セットアップ | 🔧 進行中 |
| **Phase 2** | リモート管理 — SSH接続, ステータス監視 | ⏳ |
| **Phase 3** | スマート機能 — コンテキスト管理, セッション | ⏳ |
| **Phase 4** | マルチAI連携 — GPT-4o, Gemini Flash統合 | ⏳ |
| **Phase 5** | 磨き込み — Notion連携, UI/UXブラッシュアップ | ⏳ |

詳細は [Notion 設計書](https://www.notion.so/3027e62888da81f98abee4560ceb6850) を参照。

---

## 💻 環境状況 / Environment Status

| 項目 | OMEN (司令塔) | SIGMA | Precision |
|---|---|---|---|
| Claude Code | ✅ v2.1.37 | ❌ | ❌ |
| Node.js | ✅ v20.19.2 | ❌ | ❌ |
| Rust | ✅ 1.93.0 | — | — |
| VS Build Tools | ✅ 2022 | — | — |
| Tauri CLI | ✅ 2.10.0 | — | — |
| SSH Server | — | ❌ | ✅ (停止中) |

---

## 🔑 APIキー管理 / API Key Management

APIキーは `.env` ファイルまたは OS 環境変数で管理。**Git には絶対にコミットしない。**

```bash
# .env (例)
OPENAI_API_KEY=sk-...
GOOGLE_AI_API_KEY=AI...
```

---

## 🛠 開発コマンド / Development

```bash
cd nexus-app

# 開発モード（ホットリロード付き）
npm run tauri dev

# リリースビルド
npm run tauri build

# Rust のみビルド確認
cd src-tauri && cargo build
```

---

## 📝 ライセンス / License

Private project.

---

## 🔮 将来構想 / Future Vision

- PC追加（新マシンをSSH登録するだけでスケール）
- AI追加（Perplexity, DeepSeek, Mistral 等）
- モバイル対応（スマホからタスク確認）
- 音声入力対応
- タスク自動スケジューリング
