# 🚀 Project Nexus — 次スレ引き継ぎ

## プロジェクト概要
マルチPC × マルチAI オーケストレーションハブ。OMENを司令塔として全PC・全AIを一つのデスクトップアプリから統括操作するシステム。Tauri (Rust + HTML/CSS/JS) で構築。

## リポジトリ・ドキュメント
- GitHub: `https://github.com/yakizakanateisyoku/project-nexus`
- ローカル: `C:\Users\annih\Documents\GitRepository\project-nexus`
- Notion設計書: `https://www.notion.so/3027e62888da81f98abee4560ceb6850`

## 開発環境 (OMEN)
- Claude Code v2.1.37, Node.js v20.19.2, Rust 1.93.0, VS Build Tools 2022, Tauri CLI 2.10.0
- PowerShell注意: `&&` 使えない、`;` で連結。cargo PATHは `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"` が必要な場合あり
- ビルド: `cd nexus-app; npm run tauri dev`
- ビルド前に古いプロセスが残っていたらkillすること（exe lockされる）: `taskkill /f /im "nexus-app.exe"`

## Phase 1 ✅ 完了
- ✅ 環境セットアップ (OMEN)
- ✅ Tauriプロジェクト作成・ビルド確認
- ✅ README.md強化（日英バイリンガル）
- ✅ チャットUI実装
  - ダークテーマ (`--bg-primary: #1a1b1e`, `--accent: #4dabf7`)
  - 左サイドバー: PC接続ステータス (OMEN/SIGMA/Precision)
  - メインエリア: メッセージバブル (user/assistant/system)
  - ヘッダー: コンテキスト使用率バッジ
  - vanilla JS（フレームワーク不要、軽量）
  - Rust側: `send_message`、`get_machine_status` コマンド
  - tokio依存なし（軽量化）
  - ウィンドウ: 1000x700、最小600x400
- ✅ システムトレイ常駐
  - ×ボタン → ウィンドウ非表示（トレイ格納）
  - 右クリックメニュー: 表示 / 終了
  - ダブルクリック → ウィンドウ復帰
  - 終了: hide→300ms猶予→std::process::exit(0) でWebView2負荷軽減
  - features: `tray-icon`, `image-png`（tokio不要を維持）

## Phase 2 ← 現在の作業

### ✅ Claude Code CLI統合（コード実装済み・動作未確認）
`send_message`をモックから実CLI呼び出しに置き換え済み。**ビルドは通る**が、実際にCLI応答が正常に返るかの動作確認が未完了。

**実装内容:**
- `std::process::Command` で `claude -p "メッセージ"` を実行
- `tauri::async_runtime::spawn_blocking` で別スレッド化（UIフリーズ防止、tokio追加不要）
- Windowsの PATH 競合対策: npmの `claude.cmd` をフルパスで指定
  - 理由: `cmd /c claude` だと Claude Desktop アプリ (`AppData\Local\AnthropicClaude\claude.exe`) が先に見つかってしまう
  - 解決: `%USERPROFILE%\AppData\Roaming\npm\claude.cmd` をフルパスで呼び出し

**⚠️ 次スレでやること:**
1. **動作テスト**: `tauri dev`でアプリ起動→メッセージ送信→Claude Code CLIの応答が表示されるか確認
2. もし応答が空や異常なら:
   - まずPowerShellで手動テスト: `cmd /c "$env:USERPROFILE\AppData\Roaming\npm\claude.cmd" -p "hello"`
   - 認証状態確認: `claude.cmd` が認証済みか（初回は `claude` で対話ログインが必要な場合あり）
   - stderr の内容を確認（Node.jsのキャッシュ警告は無視してOK）
3. 動作確認できたらコミット

### ⬜ 未着手
- SSH経由リモートPC管理（SIGMA/Precision）
- SIGMA/Precision環境セットアップ（物理アクセス時）

## TODO（軽微・後回し）
- トレイ終了時の音途切れ改善: WebViewのURLを`about:blank`に切り替え→delay→exitを試す

## Phase 3（予定）
- ⬜ コンテキスト残量表示（UI上のバッジを実データ化）
- ⬜ 自動スレッド切り替え
- ⬜ **作業中断チェック＆自動引き継ぎ生成機能**
  - コンテキスト残量を監視
  - 閾値を下回ったら作業中断を検知
  - 次スレッド用の引き継ぎを自動生成
  - 新スレッドで自動継続

## Phase 4以降
- Phase 4: マルチAI連携 (GPT-4o, Gemini Flash)
- Phase 5: Notion連携、UI/UXブラッシュアップ

## 設計メモ
- **軽量方針**: 現在tokio不要を維持。ストリーミング応答（リアルタイム表示）が欲しくなったらtokio導入＋stdout逐次読み取りに差し替え可能。`send_message`の中身を変えるだけで構造変更は不要。
- **PATH競合**: Windowsでは Claude Desktop (claude.exe) と Claude Code CLI (claude.cmd) が共存するためフルパス指定が必須

## 主要ファイル構成
```
nexus-app/
├── src/
│   ├── index.html    # チャットUI構造
│   ├── styles.css    # ダークテーマCSS
│   └── main.js       # チャットロジック (Tauri invoke)
├── src-tauri/
│   ├── src/lib.rs    # Rustバックエンド (send_message=CLI呼び出し, get_machine_status, tray)
│   ├── src/main.rs   # エントリポイント
│   ├── Cargo.toml    # features: tray-icon, image-png
│   └── tauri.conf.json  # ウィンドウ・アプリ設定
└── package.json
```

## 注意事項
- git操作はPowerShellで `;` 区切り
- ビルド前にexeロック確認必須
- cargo PATH: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"` が必要
- サーバー負荷を考え軽量に動かす（tokio不要を維持）
- 編集後はSourceTreeに反映
