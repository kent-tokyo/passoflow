# PassoFlow ロードマップ

更新日: 2026-09-06。現在のバージョン: **0.1.2**。

## 製品の方向性

PassoFlow は個人用・ローカル実行の自動化ツールです。主な評価軸は GUI 操作、画像認識、シナリオ作成の容易さ、再実行性、安全性、導入の容易さです。

オーケストレーション、深いデータワークフロー、分散実行、クラウド運用、チーム管理、監査基盤は対象外とします。

## 目標アーキテクチャ

Rust をシナリオモデル、実行エンジン、入力制御、画面取得、画像認識の正規実装にします。PassoFlow は PyAutoGUI や他の汎用GUI自動化ライブラリに依存しません。Python はPyO3による公式binding・互換インターフェースとして維持し、別実装にはしません。現在のUI設計、操作の流れ、アクセシビリティ、視認性は製品要件として移行中も維持し、既存のReactエディタはRustバックエンドが安定するまで使い続けます。

```text
passoflow-core       シナリオモデル、検証、変数、診断
passoflow-engine     実行状態、リトライ、安全制御、復旧、イベント
passoflow-input      PassoFlow独自のマウス・キーボード・ウィンドウ操作
passoflow-capture    PassoFlow独自の画面・ウィンドウ取得
passoflow-vision     PassoFlow独自のregion検索・画像認識
passoflow-web        Rust側のDOMブラウザ操作
passoflow-python     PyO3 binding、PyPI公開
passoflow-server     既存Reactエディタ向けローカルAPI
```

全レイヤーで`Scenario`、`Step`、`Action`、`Diagnostic`、`ActionResult`、`RetryPolicy`、`FailureArtifact`、`RunEvent`という型概念を共有します。

## 現在の基盤

- [x] 検証、Undo/Redo、ループ、分岐、画像取得、テーブルプレビュー、ログ、再実行を備えたブラウザエディタ。
- [x] 画面画像方式とDOM方式を分離したブラウザ操作。
- [x] 画像候補、信頼度、リトライ、オフセット、region、対象ウィンドウ確認、セレクタ支援、失敗証跡。
- [x] 初回成功、セットアップ、実行、復旧時間のローカル限定計測。
- [x] Windowsでbatをダブルクリックするだけでローカルサービスを起動し、準備完了後にブラウザを開くランチャー。
- [x] 英語・日本語・中国語のUI文字列。

## 段階的な実装計画

### Phase 0 — 契約と依存境界

- [ ] YAMLスキーマ、予約キー、アクション名、3状態のアクション結果契約を固定する。
- [x] engine、Python binding、ローカルUI間のバージョン付きRust/JSONイベント仕様を定義する。
- [x] MIT OR Apache-2.0のライセンス情報を追加し、著作権者名を残す。
- [x] 現行runnerを変更せず、Rust workspace、MSRV方針、CIマトリクスを追加する。
- [x] 現在PyAutoGUIが担う機能を全て列挙し、PassoFlow独自の置換インターフェースを定義する。

完了条件: 契約と置換範囲を文書化し、既存アクションのフィクスチャで境界をカバーする。

### Phase 1 — `passoflow-core`

- [x] Rustでシナリオ、ステップ、アクション、変数、診断、リトライ方針の基礎型を定義する。
- [x] YAML解析、予約キー検証、決定的な正規化の初期実装を追加する。
- [x] 既存形式シナリオ、未知アクション・パラメータ、診断の決定性を確認する初期Rustテストを追加する。
- [x] 制御フローと基本スキーマエラーを共有するRust/Python互換フィクスチャを追加する。
- [x] 現行runnerの依存関係が導入済みの場合に実行するPython側互換テストを追加する。
- [x] 画像の安全性、座標、信頼度、タイムアウト制約まで共有フィクスチャを拡張する。
- [x] DOMタイムアウト、Webhook payload、条件値の制約を共有フィクスチャで一致させる。
- [x] シナリオを検証して機械可読な診断を出力するRust CLIを追加する。

完了条件: Rust検証結果がPython validatorとフィクスチャ群で一致し、GUI・OSに依存しない。

### Phase 2 — 互換ブリッジ

- [x] Python runnerが解析・検証に`passoflow-core`を呼び出すようにし、既存コマンドを維持する（オプトインbridge。既定はPythonのまま）。
- [x] PythonとRustの正規化シナリオ・診断を比較する互換テストを追加する。
- [x] ステップ順序、分岐、ループ、変数参照を表すRust `ExecutionPlan`の初期境界を定義する。
- [x] ネイティブ画像・ブラウザexecutorへアクション値を渡せるよう、正規化済みstepパラメータをplanへ含める。
- [x] オプトインrunnerで実行前に計画とステップの整合性を確認する安全ゲートを追加する。
- [x] planのaction parameterに対する非破壊のRust側`{{variable}}`展開を、入れ子のYAML値を含めて追加する。
- [x] Rust execution planから分岐境界と連続したloop範囲を決定的に取得できるようにする。
- [x] 分岐判定を適用し、固定回数loopを展開する純粋なRust selectorを追加する（table loopは実行時データに委ねる）。
- [x] selectorを互換性のある`passoflow-engine::run_selected`実行経路へ接続し、既存と共通のretry・停止・event処理を利用する。
- [x] 変数のtruthiness、scalar equality、`last_step`分岐条件をRust互換で評価する処理を定義する。
- [x] Rust selectorで実行時table行を受け取り、展開したloop iteration間の行変数を分離する。
- [x] `passoflow-engine::run_selected_with_tables`を公開し、解決済みtable loop stepを共通のretry・停止・event処理で実行できるようにする。
- [ ] 変数と制御フロー計画をRust engineのインターフェースの背後へ移す。
- [ ] YAML、ログ、スクリーンショット、再実行、既存Web UIの挙動を維持する。

完了条件: 既存シナリオをスキーマ上の差異なく検証・実行できる。

### Phase 3 — PassoFlow独自入力ライブラリ

- [x] ポインタ移動、クリック、ダブルクリック、スクロール、キー、ホットキー、文字入力のRust抽象を実装する。
- [x] 座標範囲、フェイルセーフ座標、クリック表示時間、メモリ上の記録backendを追加する。
- [x] 表示倍率、マルチモニタのディスプレイ情報、キーボード配列情報、アクセシビリティ権限状態を型定義する。
- [x] 画面座標のポインタ、クリック、スクロール、キー、ホットキー、Unicode文字入力を行うWindows `user32` adapterの初期実装を追加する。
- [x] Unicode文字を8bitのkeybdイベント経路ではなく、Win32 `SendInput`のUTF-16イベントで入力する。
- [x] 仮想デスクトップ範囲の自動設定とキーボード配列情報取得を備えたWindows controller生成を追加する。
- [x] Windowsのアクティブウィンドウ座標を変換し、負のモニタ原点を含む仮想デスクトップ範囲を扱う。
- [x] 対象window操作向けに、タイトルと画面座標の矩形を返すWindows foreground-window snapshot APIを追加する。
- [x] `windows-latest`上でネイティブadapterをコンパイル・検査するWindows CIジョブを追加する。
- [ ] 対応OSのネイティブAPIを直接使うアダプタを実装し、PyAutoGUI、Enigo、他の自動化ライブラリをラップしない。
- [ ] 座標系、表示倍率、マルチモニタ、キーボード配列、権限エラーを定義する。
- [x] 独立したdry-run入力テストツールと共通安全テストを追加する。
- [ ] 赤丸表示とfail-safeをPassoFlow独自機能として維持する。

完了条件: 指定したOSで基本入力が動作し、権限・座標系の失敗を明示できる。

### Phase 4 — PassoFlow独自画面取得・画像認識

- [x] PyAutoGUI/Pillowに依存しないRGBAフレーム、画面原点、capture region、決定的cropの共通契約を定義する。
- [x] PyAutoGUI/Pillowに依存しないWindows GDI画面取得adapterの初期実装を追加する。
- [x] region境界、信頼度、候補順位、アンカー/offsetによるクリック点を決定的に扱い、曖昧な結果を安全に返すテンプレート検索を追加する。
- [x] 画面座標で取得するcapture境界と、Windows native fast path、決定的crop fallbackを追加する。
- [x] Windows foreground/native window取得と、window単位のDPI取得primitiveを追加する。
- [x] GDI画面取得可否を変更なしで検査し、granted/denied/unknownを明示する診断を追加する。
- [x] Windowsのモニタごとの列挙と、best-effort effective DPI情報を追加する。
- [ ] 対応OS全体の実行時権限案内を追加する。
- [x] `NotFound`だけを再試行する処理をRustのcapture/vision境界へ移植する。
- [x] OS非依存のアクティブウィンドウregion解決と、対象タイトル不一致時のfail-closedガードを追加する。
- [x] 画像一致のアンカー/offset結果をPassoFlow input controllerへ接続し、guard付き単/double clickを実装する。
- [x] 同じguard付きcapture/searchパイプラインを`move_mouse_to_image`にも使い、クリックなしの移動を実装する。
- [x] Rust画像executorからstepごとの`click_indicator_duration`をinput controllerへ渡す。
- [x] 検索設定とクリック設定を注入できるcapture-search-guard-click一体型パイプラインを公開する。
- [x] Windows adapterのwindow情報をvisionの安全ポリシーへ変換する境界を追加する。
- [ ] 決定的なテンプレート検索の表示倍率・色空間の仕様を定義する。
- [x] exact pixel、RGBのみ、暗黙の倍率変更なしという初期検索仕様を定義する。
- [x] 固定画像の精度テストと依存の少ない検索benchmarkを追加する。
- [x] 曖昧な一致や信頼度未満では自動クリックしない安全制御を維持する。

完了条件: Rust方式が現行の精度目標と再現可能なローカル測定を満たす。

### Phase 5 — Rust DOM・統合エンジン

- [x] リトライ、警告継続、失敗停止、停止要求、バージョン付きstep-attemptイベントを扱う、OS非依存の`passoflow-engine`初期runnerを実装する。
- [x] セレクタ・URL・timeoutの安全性検証と記録backendを備えた、OS非依存のDOMアクション境界を定義する。
- [ ] CDPまたは保守されたRustクライアントによるChromiumアダプタを選定・試作する。
- [ ] 遷移、クリック、入力、待機、セレクタプレビュー、復旧診断を移植する。
- [x] リトライ、安全停止、構造化イベントを扱う`passoflow-engine`初期実行境界を実装する。
- [x] template読み込みを注入でき、capture/search/clickポリシーを使うRust `click_image`初期`StepExecutor`を追加する。
- [x] 同じregion、retry、confidence、target-window安全ポリシーで`move_mouse_to_image`を実行する。
- [x] シナリオroot配下のPNG/JPEG/BMPを読み込み、path traversalを拒否する`FileTemplateLoader`を追加する。
- [ ] Python側のオーケストレーションをRust engineへ移し、未対応のOS部分だけ明示的なアダプタとして残す。
- [ ] 既存Reactエディタと互換性のある`passoflow-server`をローカル専用APIとして追加する。
- [ ] 現在のパレット、キャンバス、パラメータパネル、ダイアログ、ログ、フォーカス操作、ライト/ダーク表示を操作面・見た目ともに互換維持する。

完了条件: 成功、警告、失敗、停止、復旧のシナリオがRust engineで一貫して動作する。

### Phase 6 — Python bindingと公開crate

- [x] PyO3/maturinの初期`passoflow-python` bindingを作り、正規化YAML、診断、execution plan、契約エラーを公開する。
- [x] Python型stubとRust/Pythonで同じ契約動作をする例を用意する。
- [x] 再利用可能な`passoflow-core`をcrates.ioへ公開する。engineはインターフェースが安定してから公開する。
- [x] `CARGO_REGISTRY_TOKEN`を使い、`passoflow-core`から`passoflow-python`の順に公開するGitHub Actions経路を追加する。
- [ ] platform wheelをPyPIへ公開し、クリーンな仮想環境で導入を検証する。
- [x] `release.yml` / `pypi` environmentを使う`passoflow`向けGitHub Actions Trusted Publisher登録を完了する。
- [x] デスクトップ権限、Chromium、対応OS、未対応アダプタを明記する。

完了条件: Rust利用者はcrates.ioからcoreを使え、Python利用者はRustをローカルコンパイルせずwheelから同じcoreを使える。

### Phase 7 — 次期候補リリースと移行

- [ ] 互換性、セキュリティ、パッケージング、UI smoke、OS別検査を実行する。
- [ ] 固定したローカル手順で画像精度、入力遅延、初回成功、セットアップ完了、復旧時間を測定する。
- [ ] Rust相当機能の検証後に限り、Pythonだけの実装経路をdeprecatedにする。
- [ ] 移行メモ、CHANGELOG、crates.io公開、PyPI公開、次期バージョン候補を準備する。

完了条件: 公開成果物、文書、バージョン情報、UIが同じサポート範囲を示す。

## 対象外

- 分散実行、クラウドオーケストレーション、チーム管理、監査基盤、深いデータパイプライン機能。
- PassoFlowのシナリオモデルと無関係な広範なPyAutoGUI互換API。
- バックエンドをRust化するためだけのReactエディタ全面書き換え。
- エンジン移行中に現在のUI設計を別の操作パラダイムへ置き換えること。

## 完了基準

各Phaseは、実装、互換フィクスチャ、利用者向け文書、安全性、パッケージング、UI回帰の証拠が一致した場合のみ完了とします。性能主張には固定したローカル測定を、対応OSの主張には明記した環境での実行証拠を必要とします。Rust移行によって、現在の使いやすさ、視認性、キーボード操作、ライト/ダーク表示の可読性を悪化させないことを必須とします。
