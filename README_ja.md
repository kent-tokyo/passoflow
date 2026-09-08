# PassoFlow

[![CI](https://github.com/kent-tokyo/passoflow/actions/workflows/quality.yml/badge.svg)](https://github.com/kent-tokyo/passoflow/actions/workflows/quality.yml)
[![Docs](https://img.shields.io/badge/docs-user%20guide-4c8bf5)](https://github.com/kent-tokyo/passoflow/blob/main/docs/guide.md)
![Version](https://img.shields.io/badge/version-0.1.3-blue)

Windows向けのRPAツール。画面上の画像を検索してマウス操作(移動・クリック・ダブルクリック)を行うほか、キーボード入力・クリップボード操作・ウィンドウ前面化・アプリ起動・ファイル操作・Excel/CSVの読み書きも行える。操作の並びはYAMLシナリオファイルで記述する。

PassoFlow は個人用・ローカル実行を前提とした自動化ツールです。評価と開発の中心は、GUI操作、画像認識、シナリオ作成、再実行性、安全性、使いやすさに置きます。

Rustのシナリオ契約は`passoflow` package（Pythonからのimport moduleは`passoflow_python`）で利用できます。公開wheelは`python -m pip install passoflow`で導入でき、開発時は`rust/crates/passoflow-python`で`maturin develop`を実行します。bindingにはローカルChromium CDPを直接操作する`DomBrowser`と、ネイティブデスクトップ対応状況を読み取り専用で確認する`input_platform_info_json()`があります。接続方法は[`rust/crates/passoflow-python/README.md`](rust/crates/passoflow-python/README.md)、契約APIの最小例は[`examples/python_binding.py`](examples/python_binding.py)を参照してください。

Rust移行の段階と、PyAutoGUIに依存しないPassoFlow独自実装の境界は、[Rustエンジン境界](docs/rust-engine.md)に記載しています。共有するYAMLとアクション結果の契約は[シナリオ契約](docs/scenario-contract.md)に要約しています。各Phaseの完了条件を満たすまで、現在のPython runnerを互換基準として維持します。

オーケストレーション、分散実行、クラウド運用、チーム管理、監査機能は優先しません。Excel/CSV対応も、GUIシナリオを支える実用的な範囲に限定し、独立したデータワークフロー基盤へ深く拡張することは目指しません。

プロダクトの最優先事項は次の2つです。

1. 画面エディターで、動くGUIフローを簡単に作れること
2. 個人のローカル環境へ簡単に導入し、すぐ使い始められること

初回利用の流れは、PassoFlowを起動する → シナリオを開く、または新規作成する → アクションをクリックまたはドラッグで追加する → 必須値を設定する → 実行する → 結果を理解する、という短い経路を目指します。

[English version here](README.md) | [CHANGELOG](CHANGELOG.md) | [SECURITY](SECURITY.md)

ライセンスは [MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) です。

## まず使ってみる

PassoFlow は、Windows上の個人の作業を画面から自動化するツールです。シナリオを作るために、最初からYAMLを編集する必要はありません。

1. `pip install -r requirements.txt` でPythonの依存関係をインストールします。
2. `web-ui`で `npm ci` を実行します（ソースコードから使う場合のみ）。
3. Windowsで `run_webapp.bat` をダブルクリックします。起動完了後にブラウザが自動で開きます。
4. エディターで既存のシナリオを開くか、新しいシナリオを作成します。
5. 左のアクションをクリックするか、キャンバスへドラッグします。
6. 右のパネルで必須値を入力し、「保存」を選んでから「実行」を押します。

初めて使う方は [一般ユーザー向けマニュアル](http://127.0.0.1:8000/docs/manual?lang=ja) から始めてください。YAMLを直接編集するときや、全パラメータを確認したいときだけ [上級者向け YAML / アクションリファレンス](http://127.0.0.1:8000/docs/manual?lang=ja&audience=advanced) を参照してください。

## セットアップ

WindowsおよびPython 3.10以降が必要。Excelを画面上で開くアクションやVBAマクロを実行するアクション以外は、Microsoft Excelをインストールせずに利用できる。

```
pip install -r requirements.txt
python -m playwright install chromium  # browser_*のDOMアクションを使う場合のみ必要
```

`.env.example`を`.env`にコピーして`ANTHROPIC_API_KEY`を設定すると、Web UIのAIチャット・提案・フィードバック機能が使えるようになる(任意 — それ以外の機能はこの設定なしでも使える)。

## ディレクトリ構成

| パス | 内容 |
| --- | --- |
| `src/screen_actions.py` | 画像検索・マウス操作(移動・クリック・ダブルクリック)の本体関数 |
| `src/input_actions.py` | キーボード入力・クリップボード・ウィンドウ前面化・アプリ起動・Excel/CSVの読み書きの本体関数(画像検索を伴わないもの) |
| `src/overlay.py` | シナリオ実行中に表示する常に最前面の画面オーバーレイ(対象ウィンドウの枠、現在のステップを示すHUD) |
| `src/run_scenario.py` | YAMLシナリオを読み込んで順に実行するランナー |
| `src/api_server.py` | Web UI用のFastAPIバックエンド(シナリオCRUD、実行のストリーミング、AI機能) |
| `src/logging_config.py` | ログ設定(`logs/`にファイル保存 + コンソール出力) |
| `src/test.py` | 動作確認用の手動テストスクリプト |
| `examples/python_binding.py` | Rust版Python bindingの最小利用例 |
| `scenarios/*.yaml` | シナリオ定義ファイル |
| `scenarios/images/<シナリオ名>/*.png` | シナリオで使うテンプレート画像 |
| `logs/` | 実行ログ(gitignore対象) |

## 実行方法

Web UIでシナリオを作成・保存した後、`scenarios/`配下のYAMLを実行します。

```
python src\run_scenario.py scenarios\my_scenario.yaml
```

## ドキュメント

画面から操作する方は [一般ユーザー向けマニュアル](http://127.0.0.1:8000/docs/manual?lang=ja) を参照してください。[上級者向け YAML / アクションリファレンス](http://127.0.0.1:8000/docs/manual?lang=ja&audience=advanced) には、全アクション、パラメータ、変数、ループ、分岐、バリデーションを記載しています。

## Web UI (シナリオエディタ)

ブラウザで動くビジュアルエディタ(`web-ui/`、React + Viteアプリ)。バックエンドは`src/api_server.py`(FastAPI)。シナリオ編集、画像のキャプチャ・切り抜き、テーブルの取り込み・プレビュー、ループ、条件分岐、Undo/Redo、実行ログに対応している。テーブル取り込み時は実行用の内部`load_table`ステップを自動作成または更新するが、キャンバスには表示しない。Windowsでは`run_webapp.bat`をダブルクリックするだけで、必要なサービスを起動してブラウザを開ける。`web-ui/dist`がある配布版はFastAPIだけを起動し、ソース checkout ではFastAPIとViteを起動する。macOSではPythonバックエンドの依存関係が利用できる場合に`run_webapp.command`をダブルクリックすると起動できる。ただし、Win32やExcel COMを使うアクションを含むため、RPA実行環境はWindows向けであり、macOS用スクリプトだけでクロスプラットフォーム対応になるわけではない。詳しくは [Web UI開発ガイド](web-ui/README.md) を参照する。手動で起動する場合は:

ブラウザ操作は2方式から選べる。画面認識方式は`open_url`の後に`activate_window`と`click_image`/`type_text`を使い、既定ブラウザを画面として操作する。DOM方式は`browser_navigate`、`browser_click`、`browser_fill`、`browser_wait_for`を使い、別のPlaywright管理ブラウザをCSSセレクタで操作する。

Rust validatorは現在オプトインです。`cargo build --manifest-path rust/Cargo.toml --bin passoflow-validate`でビルドし、生成されたバイナリを`PASSOFLOW_VALIDATE_BIN`に設定したうえで、ローカルrunnerの試行時だけ`PASSOFLOW_USE_RUST_VALIDATOR=1`を設定してください。両方を設定しない場合は、従来どおりPython validatorを使用します。Rustの実行計画は`passoflow-validate --plan <scenario.yaml>`で確認できます。

```
python src\api_server.py
cd web-ui && npm run dev
```

Web UIの変更を提出する前に、`web-ui`ディレクトリで`npm run build`、`npm run lint`、`npm run test:smoke`を実行する。スモークチェックは追加のテストランナーなしでエディタの主要な契約を検査する。

Web UIにはClaude APIを使ったAIチャットアシスタントがある。アクションを選択した状態で「入荷番号という変数を入力欄に貼り付けたい」のように具体的に依頼すると、必要なアクション列(通常は`set_clipboard`と`paste`)を作成し、選択中ノードの直後に挿入する。挿入時は後続ノードの配置を自動調整し、Undoで取り消せる。その他に、各ノードの右クリックメニューから使える「AIに相談」機能(そのステップのパラメータ設定や、より適切なアクションへの変更)と、実行後に警告があった場合の改善提案フィードバックがある。これらのAI機能には`.env`の`ANTHROPIC_API_KEY`が必要(セットアップ参照) — それ以外のエディタ機能はこの設定なしでも使える。

## ログ

実行するたびに`logs/<name>_<日時>.log`が作成される。各ステップの実行内容・検索結果(見つかった座標 / 見つからなかった旨)が記録されるので、後から動作を確認できる。
