# アクション一覧

[English version here](actions.md)

`confidence`・`offset`・`position`・`retry`・`retry_interval_ms`は画像検索を伴うアクションに共通のオプション。`activate_window`も`retry`・`retry_interval_ms`に対応する(画像検索を行わないため`confidence`・`offset`・`position`はない)。

## 目的別クイックスタート

どのアクションを選べばよいか分からない場合は、まず目的に合う行から
始めてください。下の一覧は、各パラメータの詳細リファレンスです。

| 目的 | 最初に使うアクション | 代表的な次の操作 |
| --- | --- | --- |
| アプリを起動して操作する | `launch_app` | 起動完了を確認する場合は`wait_for_window`を設定し、`activate_window` → 画面操作またはキーボード操作 |
| ボタンやメニューをクリックする | `click_image` | 対象画像を取得し、画面表示が遅い場合は `retry` を設定する |
| 文字を入力する | `type_text` | 変数の値を使う場合は `{{変数名}}` を使う |
| Excel のセルを読み書きする | `get_excel_value` / `set_excel_value` | ファイル、シート、セルを設定し、変化する値は変数にする |
| Excel / CSV の選択行を処理する | `load_table` | Web UIで取り込み、シナリオの「開始」〜「終了」をテーブルループにする |
| 条件に応じて処理する | `if` | 変数と必要に応じて `equals` を設定し、条件内に処理を置く |
| 既存シナリオを再利用する | `call_scenario` | 参照先を複数回実行する場合は `repeat` を使う |

### 基本的な操作手順

1. アクション一覧で、やりたいことを検索する。
2. アクションをキャンバスへドラッグするか、クリックして選択中のステップの直後に追加する。追加したアクションは自動的に選択される。
   ノードにフォーカスがある場合は、矢印キーで近くのノードへフォーカスを移動できる。Alt+矢印キーで配置を微調整でき（Shiftで大きく移動）、操作を使い分けられる。
3. 追加したノードを選択し、パラメータパネルの必須項目を入力する。
4. アクション名だけでは目的が分かりにくい場合は、タイトルやメモを追加する。
5. シナリオを保存して実行し、実行ログを確認する。

警告が出た場合は、まずログに表示されたステップを確認してください。
画像やウィンドウが見つからない場合は、取得した画像を確認し、適切な
`wait` を追加するか、`retry` を増やすと改善することがあります。

Webのアクション一覧は目的別に、フロー、制御、アプリ操作、ファイル操作、
Excel、画面操作、変数、キーボード入力のカテゴリに分かれています。
ファイル操作には`rename_file`・`move_file`・`copy_file`が含まれます。
アプリの起動とウィンドウの前面化はアプリ操作に残ります。

| action | 必須パラメータ | 動作 |
| --- | --- | --- |
| `call_scenario` | `path` | 別のYAMLシナリオファイルをその場で実行する。変数は呼び出し元と共有される |
| `repeat` | `path`, `count` | 別のYAMLシナリオファイルを`count`回連続でその場で実行する。変数は呼び出し元・各回の間で共有される |
| `send_webhook` | `url` | HTTPリクエストを送信する。`method`は省略時`POST`、`payload`はJSONとして送信する。送信必須なら`on_error: stop`を指定し、デフォルトの`continue`では警告のみで継続する |
| `if` | `variable` または `last_step` | 条件分岐を開始する。`variable`と任意の文字列比較`equals`、または直前のアクションを調べる`last_step: ok`/`warned`を使う。`endif`と対にする必要があり、間に`else`を挟むこともできる — 詳細は下記の分岐の項を参照 |
| `else` | なし | 直近の開いている`if`の偽の分岐の開始を示す。省略可能 — `else`のない`if`は条件が偽の場合何もしない |
| `endif` | なし | 直近の開いている`if`・`else`ブロックの終わりを示す |
| `set_variable` | `name`, `value` | 変数を保存する。`name`は自由な文字列(日本語可) |
| `concat_variable` | `name`, `value` | `set_variable`と同様だが、`value`は先に`{{...}}`プレースホルダが解決される — 他の変数同士や変数と固定文字を連結できる |
| `set_year_month_variable` | `name` | 今日(またはオフセット後)の年月を`YYYYMM`形式で変数に保存する。`days_offset`・`months_offset`で調整できる |
| `set_year_month_day_variable` | `name` | 今日(またはオフセット後)の日付を`YYYY/MM/DD`形式で変数に保存する。`days_offset`・`months_offset`で調整できる |
| `set_month_start_variable` | `name` | 対象月の1日を`YYYY/MM/01`形式で変数に保存する。`days_offset`・`months_offset`で調整できる |
| `set_month_end_variable` | `name` | 対象月の実際の末日(28〜31日、今日が何日でも正しく計算される)を`YYYY/MM/DD`形式で変数に保存する。`days_offset`・`months_offset`で調整できる |
| `type_text` | `text` | フォーカスされている要素にテキストを入力する(クリップボード経由。日本語などUnicodeにも対応) |
| `set_clipboard` | `text` | クリップボードに文字列をコピーする |
| `paste` | なし | クリップボードの内容をCtrl+Vで貼り付ける |
| `paste_variable` | `name` | 変数の値を`{{...}}`テンプレートを使わずに直接貼り付ける(クリップボード経由、`type_text`と同じ仕組み) |
| `clear_input` | なし | フォーカスされている要素の内容を消去する(Ctrl+Aで全選択してからDelete) |
| `press_key` | `key` | 1つのキーを押す(例: `enter`・`tab`・`esc`)。その後`wait`ミリ秒(デフォルト100)待機する |
| `hotkey` | `keys` | 複数のキーを同時に押す(例: `[ctrl, v]`でCtrl+V) |
| `launch_app` | `path` | 指定した実行ファイルを起動する。`wait_for_window`を指定するとウィンドウタイトルを待ち、早期終了を検出する。`startup_timeout_ms`で待機上限を設定できる。`args`で引数リストを渡せる |
| `rename_file` | `path`, `new_name` | ファイルを現在のフォルダ内でリネームする |
| `move_file` | `path`, `destination` | ファイルを`destination`(完全な移動先パス)へ移動する。存在しない親フォルダは自動作成される |
| `copy_file` | `path`, `destination`, `if_destination_newer` | ファイル(`path`にワイルドカードを使う場合はマッチした全ファイル)を`destination`へコピーする。存在しないフォルダは自動作成される。`if_destination_newer`(`overwrite`/`skip`、デフォルト`overwrite`)でコピー先に既存のより新しいファイルがある場合の挙動を制御 |
| `map_network_drive` | `drive`, `path` | `net use`により`drive`(例: `Z:`)をUNCパス`path`(例: `\\server\share`)に割り当てる |
| `open_excel_file` | `path` | 指定したファイルを、関連付けられたアプリケーション(通常はExcel)で開く。ファイルをダブルクリックするのと同じ |
| `open_new_excel` | なし | Excelを新規の空白ブックで開く |
| `get_excel_value` | `path`, `cell`, `name` | `.xlsx`ファイルの1つのセル(例: `B3`)を読み取り、変数に保存する。`sheet`でシート名を指定できる(省略時はアクティブシート) |
| `set_excel_value` | `path`, `cell`, `value` | `.xlsx`ファイルの1つのセル(例: `B3`)に`value`を書き込んで保存する。`sheet`でシート名を指定できる(省略時はアクティブシート) |
| `save_excel_file` | `path` | `.xlsx`ファイルを開いてそのまま保存し直す（Microsoft Excel不要） |
| `create_excel_sheet` | `path`, `sheet` | `.xlsx`ファイルに`sheet`という名前の空シートを追加して保存する(常に末尾に追加され、位置は指定できない) |
| `delete_excel_sheet` | `path`, `sheet` | `.xlsx`ファイルから`sheet`という名前のシートを削除して保存する |
| `delete_excel_row` | `path`, `row` | `.xlsx`ファイルの`row`行目(1始まり、Excel表示上の行番号と同じ)を削除し、それより下の行を1つずつ上に詰めて保存する。`sheet`でシート名を指定できる(省略時はアクティブシート) |
| `sort_excel_range` | `path`, `range`, `key_cell` | `range`(例: `A2:C20`)を`key_cell`(例: `B2`)が含まれる列を基準に並べ替えて保存する。`range`にはヘッダー行を含めない — ヘッダー行を含めると一緒に並べ替えられてしまう。`order`は`asc`(既定)または`desc`。`sheet`でシート名を指定できる(省略時はアクティブシート) |
| `run_excel_macro` | `path`, `macro` | COM経由でブック内のVBAマクロ(Sub)を実行する — 実際のExcelインストールが必要。ファイルが既に開いていればそのブックを使い、開いていなければ開く。`args`でマクロに追加の引数を渡せる |
| `load_table` | `path`, `name` | `.xlsx`/`.csv`ファイルのヘッダー行を列名として、各行をデータとして読み込み、`name`で登録する(下記「テーブルに対するループ」参照)。`selected_rows`を指定すると、Web UIの取り込みプレビューでチェックされた0始まりの行番号だけを実行対象にできる。`sheet`でシート名を指定できる(`.xlsx`のみ、省略時はアクティブシート) |
| `activate_window` | `title_contains` | タイトルに指定した文字列を含むウィンドウを検索し、前面化する |
| `open_url` | `url` | 既定ブラウザでURLを開く。画面操作には既存の画像認識アクションを続けて使う |
| `browser_navigate` | `url` | Playwrightの別ブラウザページをURLで遷移する |
| `browser_click` | `selector` | PlaywrightブラウザでCSSセレクタのDOM要素をクリックする |
| `browser_fill` | `selector`, `text` | PlaywrightブラウザでCSSセレクタの入力欄やtextareaに入力する |
| `browser_wait_for` | `selector` | DOM要素が表示・存在・非表示・消滅するまで待つ |
| `move_mouse_to_image` | `images` | 複数の候補画像を順に試し、最初にマッチした位置にマウスを移動する |
| `click_image` | `images` | 複数の候補画像を順に試し、位置を赤丸で表示してから最初にマッチした位置をクリックする。`click_type: double`でダブルクリック、`click_indicator_duration: 0`で赤丸を非表示、0より大きい小さな値で実行を短縮できる |
| `wait` | `ms` | 指定したミリ秒だけ待機する |

### ブラウザ操作方式の選び方

画面に表示されたブラウザを操作する場合は、`open_url`の後に`activate_window`と`click_image`/`type_text`を使う。実際の画面に基づくためCanvas主体のページにも使えるが、解像度・表示倍率・見た目の変更の影響を受ける。

安定したDOMセレクタを使える場合は、`browser_navigate`、`browser_click`、`browser_fill`、`browser_wait_for`を使う。こちらは座標の影響を受けず一般に正確だが、PlaywrightとChromium（`python -m playwright install chromium`）が必要。DOMブラウザはシナリオ内のこれらのアクションで共有され、シナリオ終了時に閉じる。

エディタの「一致をプレビュー」では、URLとセレクタを指定して一時的なローカルPlaywrightページを開き、一致する要素数を確認できる。シナリオのブラウザ状態は変更しないため、提示された候補セレクタは内容を確認してから使う。

## 各アクションの詳細な挙動

| action | 詳細な挙動 |
| --- | --- |
| `call_scenario` | 対象のYAMLを読み込み、同一プロセス内でその`steps`をその場で実行する。`variables`辞書は呼び出し元と同一のオブジェクト(コピーではない)なので、呼び出し先の`set_variable`・`set_*_variable`系のステップで設定した値は呼び出し元にも反映される(逆も同様)。ネストした実行では、Web UIが実行中ステップをハイライトするために使う`@@PROGRESS@@n/total`マーカーが出力されない。ネスト先のステップ番号はサブシナリオ内での番号であり呼び出し元の番号と対応しないため、実行中はUI上`call_scenario`のステップ自体がハイライトされ続ける。 |
| `repeat` | 読み込み・変数共有・進捗マーカーの挙動は`call_scenario`と同じだが、対象の`steps`を`count`回連続で実行してから次のステップへ進む。`count`は正の整数である必要がある。 |
| `send_webhook` | `url`内の`{{...}}`プレースホルダを解決し、`payload`も再帰的に解決する。リクエスト送信にはPython標準の`urllib.request`を使い、`payload`指定時は`Content-Type: application/json`を自動設定する。失敗時はデフォルトで警告して継続し、`on_error: stop`を指定した場合はシナリオを停止する。 |
| `set_variable` | `value`をそのまま保存する。`value`自体は`{{...}}`プレースホルダの解決対象ではない(解決されるのは`type_text`・`set_clipboard`の`text`と`concat_variable`の`value`)。`name`・`value`はどちらもそのまま扱われる(日本語可)。 |
| `concat_variable` | `set_variable`と同じだが、`value`は先に`{{...}}`プレースホルダが解決される(`type_text`・`set_clipboard`と同じ`_resolve()`を使用) — 他の変数同士や変数と固定文字を連結できる。例: `{{last_name}}{{first_name}}`や`{{name}}様`。 |
| `set_year_month_variable` / `set_year_month_day_variable` / `set_month_start_variable` / `set_month_end_variable` | いずれも`datetime.now()`を基準に、まず`days_offset`(`timedelta(days=...)`)を適用し、その後`months_offset`を適用する — 月を加算する際は日を対象月の末日にクランプするため、例えば1月31日に`months_offset: -1`を指定すると3月3日ではなく2月28日(閏年なら29日)になる。そこから先の書式・上書きがそれぞれ異なる: `set_year_month_variable`は`%Y%m`で書式化。`set_year_month_day_variable`は`%Y/%m/%d`で書式化。`set_month_start_variable`は`%Y/%m/01`で書式化(「01」は固定の文字であり実際のstrftimeコードではないため、実際に計算された日が何日であっても常に01日になる)。`set_month_end_variable`は書式化する前に日を`calendar.monthrange`で求めた対象月の実際の末日(28〜31日)に上書きしてから`%Y/%m/%d`で書式化する — これが「月末」を正しく求める方法であり、`months_offset`自身のクランプ(今日の日にちがたまたま対象月の日数を超えている場合のみ末日になる)とは異なる。 |
| `type_text` | 現在のクリップボードの内容を読み取り、`text`で上書きしてCtrl+Vを送信し、その後元のクリップボードの内容に戻す。`pyautogui.write()`は物理キーボードレイアウトに存在する文字しか入力できず日本語などの非ASCII文字が文字化けするため、キー入力の模倣ではなくクリップボード経由で入力している。 |
| `set_clipboard` | `text`をクリップボードにコピーし、そのままにする(`type_text`と異なり実行後に元に戻さない)。 |
| `paste` | Ctrl+Vを送信するだけで、クリップボードの内容には手を加えない。 |
| `paste_variable` | `name`を変数辞書から直接引く(`name`自体は`{{...}}`の解決対象ではなく、`if`の`variable`と同様に変数名そのもの)。得られた値を`type_text`と同じ仕組み(クリップボードに退避・Ctrl+V・元に戻す)で貼り付ける。未設定の変数はエラーにならず空文字列として貼り付けられる。 |
| `clear_input` | フォーカスされている要素にCtrl+A、続けて`delete`キーを送信する。クリップボードには手を加えない。 |
| `press_key` | `key`をそのまま`pyautogui.press`に渡す。値はpyautoguiの`KEYBOARD_KEYS`(後述)のいずれかである必要がある。その後`wait`ミリ秒(デフォルト100)だけ待機する。これは各ステップの後に自動で適用される`STEP_DELAY`(50ms)とは別に加算される。 |
| `hotkey` | `keys`をそのまま`pyautogui.hotkey(*keys)`に渡す。複数キーは順番に押すのではなく同時に(コードとして)押される。 |
| `launch_app` | `subprocess.Popen([path, *args])`を実行する。プロセス生成に失敗するとシナリオを停止する。`wait_for_window`にタイトルの一部を指定するとアプリのウィンドウを待ち、起動直後の終了や`startup_timeout_ms`のタイムアウトもシナリオを停止する。指定しない場合はプロセス生成後に戻る。 |
| `rename_file` | `path`・`new_name`内の`{{...}}`プレースホルダを解決し、`Path(path).rename(Path(path).parent / new_name)`を呼ぶ — `new_name`はパスではなく単なるファイル名で、ファイルは元のフォルダに留まる。失敗した場合(元ファイルが存在しない、名前の衝突など)は警告ログを出すのみでクラッシュしない。 |
| `move_file` | `path`・`destination`内の`{{...}}`プレースホルダを解決し、`destination`の親フォルダが無ければ作成し(`Path(destination).parent.mkdir(parents=True, exist_ok=True)`)、`shutil.move`を呼ぶ。`destination`は常にフォルダではなく完全なファイルパス — 同名のままフォルダへ移動したい場合は`{{folder}}/元のファイル名.拡張子`の形でパスを組み立てる。失敗した場合は警告ログを出すのみでクラッシュしない。 |
| `copy_file` | `path`・`destination`内の`{{...}}`プレースホルダを解決する。`path`にワイルドカード文字(`*`, `?`, `[...]`)が含まれない場合は`move_file`と同様だが`shutil.copy2`(更新日時などのメタデータも保持する)を呼ぶ — `destination`は完全なファイルパスで、その親フォルダが無ければ自動作成される。ワイルドカードが含まれる場合は`glob.glob(path)`で展開し、マッチした各ファイルを`destination`(この場合はフォルダとして扱われ、無ければ自動作成)へ元のファイル名のままコピーする。マッチが0件の場合は警告ログを出すだけで何もコピーしない。いずれの場合も`if_destination_newer`(`overwrite`/`skip`、デフォルト`overwrite`)が、コピー先に既にファイルがありその更新日時がコピー元より新しい場合の挙動を制御する — `overwrite`は上書きし、`skip`はそのままにする。失敗した場合は警告ログを出すのみでクラッシュしない。 |
| `map_network_drive` | `drive`・`path`内の`{{...}}`プレースホルダを解決する。まず`net use <drive> /delete /y`を実行し結果を無視する(まだ何も割り当てられていない場合もあるため)。続けて`net use <drive> <path>`を実行し、こちらの終了コードが0以外の場合のみ警告ログを出す。事前に切断することでこのステップは冪等になり、同じシナリオを再実行しても「既に使用されています」で失敗しない。ユーザー名・パスワードには対応しておらず、対象共有への認証は現在のWindowsセッションの既存の認証に依存する。 |
| `open_excel_file` | `path`内の`{{...}}`プレースホルダを解決してから(例: 日付入りのファイル名を変数で組み立てられる)`os.startfile(path)`を呼ぶ — エクスプローラーでファイルをダブルクリックするのと同じ仕組みなので、そのファイルの拡張子に実際に関連付けられているアプリで開かれる(`.xlsx`なら通常Excelだが保証はされない)。すぐに戻り、アプリが開き終わるのを待たない。 |
| `open_new_excel` | `cmd`経由で`start excel`を実行する。`excel`はWindowsのApp Pathsレジストリを通じて解決される(「ファイル名を指定して実行」に`excel`と入力するのと同じ仕組み)ため、Officeのバージョンやインストール先によって異なる固定のインストールパスに依存しない。Excelがすでに起動している場合、通常は別プロセスではなく既存のインスタンス内に新しい空白ブックのウィンドウが開く。 |
| `get_excel_value` | `path`/`sheet`/`cell`内の`{{...}}`プレースホルダを解決し、ブックを開く。`sheet`が指定されていれば切り替える(存在しないシート名を渡した場合は、何も読めずに終わるのではなくエラーとして扱う)。A1形式の`cell`(例: `B3`)から行・列位置を求めて値を読み取る — 数式セルの場合はファイルに保存されているキャッシュ済みの値を返す(Excelで開いて再計算せずに読む場合と同様。数式が一度も計算されていないファイルでは空になる)。整数値は末尾の`.0`を付けずに文字列化し、空セルは`""`になる。日付書式のセルは日付文字列に変換され**ない** — Excelのシリアル値(生の数値)がそのまま返る(この改善は今後の対応予定)。ファイルの変更・保存は行わない。 |
| `set_excel_value` | `path`/`sheet`/`cell`/`value`内の`{{...}}`プレースホルダを解決し、ブックを開き、`sheet`が指定されていれば切り替える(`get_excel_value`と同じ存在チェックを行う)。セルに`value`を書き込む — 数値に見える文字列はまず数値に変換してから設定するため、そのセルを参照する他の数式が引き続き機能する。それ以外はテキストとして保存される。最後に保存する。 |
| `save_excel_file` | `path`内の`{{...}}`プレースホルダを解決し、Microsoft Excelをインストールせずにファイルを開いてそのまま保存し直す。 |
| `create_excel_sheet` | `path`/`sheet`内の`{{...}}`プレースホルダを解決し、ブックを開いて`sheet`という名前のシートを追加する(同名のシートが既にある場合は何も変わらない)、続けて保存する。新しいシートは常に末尾に追加され、位置は指定できない。またASCIIのみのシート名は保存時に小文字化されてしまう(日本語などの非ASCII名は影響を受けない。ASCII名の改善は今後の対応予定)。失敗した場合(パスが不正、ファイルが他で開かれているなど)は警告としてログに記録され、実行は止まらない。 |
| `delete_excel_sheet` | `path`/`sheet`内の`{{...}}`プレースホルダを解決し、ブックを開いて`sheet`の存在を確認したうえで削除し、保存する。失敗した場合(パスが不正、シート名が存在しない、ファイルが他で開かれているなど)は警告としてログに記録され、実行は止まらない。 |
| `delete_excel_row` | `path`内の`{{...}}`プレースホルダを解決し、ブックを開いて`sheet`が指定されていれば存在確認・選択を行い、elixceeのブックAPIで`row`行目(1始まり)を削除して保存する。行より下のセル値は詰められるが、結合セルやスタイルのメタデータは書き換えられない。失敗した場合(パスが不正、`row`が1未満、シート名が存在しない、ファイルが他で開かれているなど)は警告としてログに記録され、実行は止まらない。 |
| `sort_excel_range` | `path`/`range`/`key_cell`内の`{{...}}`プレースホルダを解決し、ブックを開いて`sheet`が指定されていれば存在確認・選択を行い、elixceeのブックAPIで`range`を`key_cell`が含まれる列を基準に指定した`order`で並べ替えて保存する。`range`にはヘッダー行を含めないこと。`order`は`asc`(既定)または`desc`。失敗した場合(パスが不正、`range`/`key_cell`が不正、シート名が存在しない、ファイルが他で開かれているなど)は警告としてログに記録され、実行は止まらない。 |
| `run_excel_macro` | `path`/`macro`/`args`の各要素内の`{{...}}`プレースホルダを解決する。他のExcelアクションと異なり、実際に画面上のExcelをCOM経由で操作する — ブック自身のVBAプロジェクトに既に保存されているマクロを実行するには、これが必要になる(他の軽量なExcelアクションではできない)。既に起動中のExcelでそのブックが開いていればアタッチを試みる(例: 直前の`open_excel_file`/`open_new_excel`ステップで開いた場合)。失敗した場合は新しいExcelインスタンスを可視状態で起動し、自分でファイルを開く。続けて`Application.Run`を呼ぶ — `macro`はSub名(`モジュール名.Sub名`も可)、`args`はそのままマクロ自身の引数として渡される。失敗した場合(マクロが見つからない、VBAエラー、Excel未インストールなど)は警告としてログに記録され、実行は止まらない。 |
| `load_table` | `path`/`sheet`内の`{{...}}`プレースホルダを解決する。`.csv`パスの場合はまずUTF-8として読み込む(`utf-8-sig`— BOMも取り除く)。UTF-8として読めない場合は、日本語の表計算ソフトで使われるCP932で再読み込みする。それ以外は指定した`.xlsx`のシート(`sheet`でシート名を指定、`.xlsx`のみ有効)を読み込む。1行目を列名として使い、それ以外の空でない各行を`{列名: 値}`の辞書にする。`selected_rows`がある場合は、指定された0始まりの行番号だけを登録し、ない場合は全行を登録する。これらは`name`のもとでメモリ上のテーブル登録簿に保存され、`loop_table`ブロックが反復処理に使う(上記「ループ」参照) — このステップ自体は変数を一切設定しない。失敗した場合(パス/シートが不正、ファイルが壊れているなど)は警告としてログに記録され、クラッシュせずに空のテーブル(0行)として登録される。 |
| `activate_window` | `pygetwindow.getWindowsWithTitle(title_contains)`でウィンドウを検索する(部分一致・大文字小文字を区別)。複数マッチした場合はOSが返した最初の1つのみを使う。すぐに見つからない場合は`retry`で指定した回数だけ追加で試行し、試行間隔として`retry_interval_ms`だけ待つ。`launch_app`の直後など、対象ウィンドウがまだ存在しないタイミングで有用。見つかった場合、ウィンドウが最小化されていれば元に戻し、前面化する直前にキー(Alt)を1回送信する — Windowsのフォーカス奪取防止機能により、バックグラウンドプロセス(Web UIからブラウザにフォーカスがある状態で開始されたこのランナーなど)の`SetForegroundWindow`呼び出しが、実際にはフォーカスを切り替えずタスクバーアイコンを点滅させるだけに黙って格下げされることがあり、しかもエラーは発生しないため、後続の画像検索がその時点で実際に画面に映っているもの(対象アプリではなくブラウザなど)に対して静かに失敗し続けてしまう — キー入力をシミュレートすると「最近の入力」とみなされ、この制限が解除される。全ての試行後もマッチするウィンドウがない場合は警告ログを出して何もしない。それでもWindowsが前面化そのものを拒否した場合(有意なエラーコードが伴わない場合もあるが、pygetwindowはそれでも例外を送出する)も、実行を中断せず警告ログを出すだけにする。 |
| `move_mouse_to_image` / `click_image` | `images`の各候補を`pyautogui.locateOnScreen`で順番に試し(pyscreezeのcv2ベースのローダーはWindows上で非ASCII/日本語のファイルパスを読めないため、パス文字列ではなくPIL経由で読み込んでいる)、画面上で最初に見つかったものの時点で停止する。一度マッチすると、それより後の候補は確認されない。対象位置はデフォルトで`position: center`。9種類の名前付きアンカーのいずれかを`position`に指定するとマッチ画像上の別の点(例: `right`で右端中央)を狙える。`offset`を指定すると代わりにマッチ画像の左上からの正確な`[x, y]`ピクセルオフセットになる(両方指定した場合は`offset`が優先)。どれもマッチせず`retry`が指定されている場合は、最後に試した候補だけでなく候補リスト全体を最初からやり直す形でリトライし、試行間隔として`retry_interval_ms`だけ待つ(デフォルトはretry=0・retry_interval_ms=500msなので、ステップで`retry`を指定しない限り従来と挙動は変わらない)。全ての試行後もどれもマッチしない場合は警告ログを出してそのステップはスキップされる — 画像が見つからないことは致命的エラーとして扱われず、シナリオは次のステップへ進む。`click_image`は実際にクリックする前に、既定値0.25秒間赤い円を表示する(常に最前面の小さなTkinterウィンドウ)。録画やデバッグ時は`click_indicator_duration`を増やし、短縮時は小さい正数、非表示時は`0`を指定できる。クリック位置を確認できる安全表示を維持しながら、通常実行の待ち時間を抑えている。`click_type: double`(デフォルト`single`)を指定するとダブルクリックになる。 |
| `wait` | `ms`ミリ秒だけ待機する。これは各ステップの後に自動で適用される`STEP_DELAY`(50ms)とは別に加算される。 |
| `start` / `end` | 何もしない。Webのフローエディタで開始・終了を視覚的に示すためだけに存在し、実行時には影響しない。 |

## 実行中の画面オーバーレイ

実行が失敗または停止した場合、PassoFlowは最終画面と、画面キャプチャが利用できれば失敗ステップの前後画像を`logs/`に保存します。保存先は実行ログに表示されます。画面内容に機密情報が含まれる場合があるため、取り扱いに注意してください。

シナリオ実行中、何を操作しているかがわかるように、常に最前面かつクリックを妨げない(下にある操作対象へそのまま通す)2種類のオーバーレイを表示する(`src/overlay.py`で実装):

- `activate_window`が直近で見つけたウィンドウを、半透明(50%)のシアン色の線で囲む。以降`activate_window`が呼ばれるたびに再描画・移動する。対象ウィンドウが自分で動いたりリサイズされたりした場合は追従しない。
- 画面の隅に現在実行中のステップ(`title`/`note`が設定されていればそれ、なければ生のアクション名)を表示する小さなHUDボックス。各ステップごとに更新される。デフォルトは画面左上だが、マウスカーソルが左上付近にある場合は右上に表示位置を変える。

どちらも実行終了時(エラー時を含む)に自動的に消える。シナリオのYAMLには一切影響せず、設定項目もない。

## AIによるフローへのアクション追加

アクションを選択した状態で**AIに相談**を開き、「入荷番号という変数を入力欄に貼り付けたい」のように具体的な追加操作を伝えると、AIが必要最小限のアクション列を作成し、選択中アクションの直後に挿入します。この例では通常、`text: "{{入荷番号}}"`を指定した`set_clipboard`と、その後の`paste`が作成されます。挿入スペースを確保するため、後続ノードは横方向の配置を維持したまま自動的に下へ移動します。挿入は1回のUndoで取り消せるため、保存前に生成されたパラメータを確認してください。

## 実行フィードバック

実行が終了した時点でWARNINGレベルのログ(画像やウィンドウが見つからなかった、など)が1件以上あった場合、Web UIの操作ログはそれらの警告をAIに送って分析させ、ログの末尾に「--- フィードバック ---」というブロックとして、次回同じ警告を防ぐための具体的な提案(該当ステップの`retry`を増やす、`wait`を追加する、候補画像を確認し直す、など)を追記する。警告が1件もない実行ではフィードバックブロックは表示されない。生成に失敗した場合(APIキー未設定、レート制限、ネットワークエラーなど)はサーバー側でログに記録した上で静かにスキップされる — 実行自体はその時点で既に終了しているため、フィードバック生成の失敗によって実行結果が失敗扱いになることはない。

## press_key / hotkey で指定できるキー

キー名はpyautoguiの`KEYBOARD_KEYS`に準拠(全194種類)。よく使うもの:

`a`-`z`, `0`-`9`, `enter`, `esc`, `tab`, `space`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `ctrl`, `ctrlleft`, `ctrlright`, `alt`, `altleft`, `altright`, `shift`, `shiftleft`, `shiftright`, `win`, `f1`-`f24`

`hotkey`は複数キーを同時に押す。

```yaml
- action: hotkey
  keys: [ctrl, v]

- action: hotkey
  keys: [ctrl, shift, esc]
```

全キー名は以下で確認できる:

```
python -c "import pyautogui; print(pyautogui.KEYBOARD_KEYS)"
```

Web UI上では、`key`・`keys`フィールドはドロップダウンから選ぶ代わりに実際にキーを押して入力することもできる — フィールド横のキーボードアイコンのボタンをクリックしてから、記録したいキー(`hotkey`の場合は組み合わせ)を押す。

## 共通オプション

| オプション | デフォルト | 説明 |
| --- | --- | --- |
| `confidence` | `0.8` | テンプレートマッチングの一致率の閾値(0〜1)。低いほど誤検出しやすい |
| `offset` | なし | `[x, y]`でマッチした画像の左上からのピクセル位置を指定する。`position`と両方指定した場合は`offset`が優先される |
| `position` | `center` | マッチした画像上の狙う位置を名前で指定する: `center`・`top`・`bottom`・`left`・`right`・`top-left`・`top-right`・`bottom-left`・`bottom-right` |
| `region` | なし | `[left, top, width, height]` の画面ピクセル範囲（任意）。対象ウィンドウやパネルに検索を絞ると高速化し、誤検出を減らせる |
| `region_origin` | `screen` | `active_window`にすると`region`を現在の前面ウィンドウ左上からの相対値として解釈する。`region`を省略すると前面ウィンドウ全体を検索する。ウィンドウ移動に強くするには、画像操作の直前に対象ウィンドウをアクティブにする |
| `target_window_title` | なし | 安全確認（任意）。検索前に前面ウィンドウのタイトルへこの文字列が含まれることを確認し、不一致なら停止する。誤ったウィンドウへのクリックが危険な場合に使う |
| `retry` | `0` | 画像(または`activate_window`の場合はウィンドウ)がすぐに見つからない場合の追加試行回数。デフォルトの`0`は1回試して諦める、つまり従来の挙動のまま |
| `retry_interval_ms` | `500` | 試行間の待機時間(ミリ秒)。`retry`が1以上の場合のみ意味を持つ。`retry`だけ指定して`retry_interval_ms`を省略しても問題ない |
| `click_indicator_duration` | `0.25` | `click_image`でクリック前に赤丸を表示する秒数。既定値は視認性を保ちつつ高速な0.25秒。録画やデバッグ時は増やし、`0`で非表示にできる |

`images`を使うアクションは、同じUI要素の見た目が状態(背景色・文字色など)によって変わる場合に、候補を順番に試すためのもの。これらのアクションで`retry`を指定した場合、リトライは最後に失敗した候補からではなく候補リストの先頭からやり直す。

```yaml
# 見つからない場合さらに3回(合計4回)、2秒間隔でリトライする
- action: click_image
  images:
    - images/example/ok.png
  retry: 3
  retry_interval_ms: 2000

# マッチした画像の右端中央(例: スクロールバーやスライダーのつまみ)をクリックしたい場合、
# 手動で[x, y]オフセットを計算する代わりにpositionを使う
- action: click_image
  images:
    - images/example/slider.png
  position: right

# シングルクリックの代わりにダブルクリックする
- action: click_image
  images:
    - images/example/folder.png
  click_type: double
```

## 変数

`set_variable`で名前(日本語可)と値を保存し、`type_text`・`set_clipboard`の`text`内で`{{変数名}}`と書くとその値に置き換わる。変数は同じシナリオの実行中だけ保持される。

```yaml
- action: set_variable
  name: 名前
  value: 太郎

- action: type_text
  text: "こんにちは{{名前}}さん"
```

他の変数や固定文字を組み合わせて新しい変数を作りたい場合は`concat_variable`を使う — `set_variable`と異なり、`value`は`{{...}}`プレースホルダが解決される:

```yaml
- action: set_variable
  name: 姓
  value: 山田

- action: set_variable
  name: 名
  value: 太郎

- action: concat_variable
  name: 氏名
  value: "{{姓}}{{名}}様"
# 氏名 は "山田太郎様" になる
```

日付を変数にしたい場合は、4つのアクションが用意されている — それぞれ書式・計算内容が異なるだけなので、`format`文字列をいじる代わりに目的に合ったものを選ぶ。`days_offset`・`months_offset`(どちらも省略可、デフォルト0)はいずれのアクションでも日付をずらせ、マイナスも指定できる。

```yaml
# 今日 (yyyy/mm/dd)
- action: set_year_month_day_variable
  name: today

# 昨日 (yyyy/mm/dd)
- action: set_year_month_day_variable
  name: yesterday
  days_offset: -1

# 今月 (yyyymm)
- action: set_year_month_variable
  name: this_month

# 先月 (yyyymm)
- action: set_year_month_variable
  name: last_month
  months_offset: -1

# 今月1日 (yyyy/mm/01)
- action: set_month_start_variable
  name: month_start

# 今月末 (yyyy/mm/dd) — calendar.monthrangeで実際の末日(28〜31日)を求める。
# 単純なmonths_offsetのクランプとは異なる
- action: set_month_end_variable
  name: month_end

# 先月末 (yyyy/mm/dd)
- action: set_month_end_variable
  name: last_month_end
  months_offset: -1
```

## YAMLの書き方

シナリオファイルはトップレベルに`steps`キーを持ち、そのリストに実行したいステップを順番に並べる。各ステップは`action`と、そのアクションに必要なパラメータを持つ。

```yaml
steps:
  # 起動直後の画面遷移を待つ
  - action: wait
    ms: 3000

  # お気に入りボタンは状態によって見た目が変わるので候補を並べる
  - action: click_image
    images:
      - images/example/favorites-1.png
      - images/example/favorites-2.png
      - images/example/favorites-3.png

  - action: wait
    ms: 1000

  # マッチした画像の左上から (10, 5) の位置をクリックしたい場合
  - action: click_image
    images:
      - images/example/list.png
    confidence: 0.9
    offset: [10, 5]
```

ポイント:

- 画像パス・`call_scenario`の`path`はいずれも`scenarios/`ディレクトリからの相対パス
- `confidence`・`offset`は省略可能。省略時は`confidence=0.8`・画像の中央をクリック
- ステップは上から順に実行され、途中で画像が見つからない場合もエラーで停止せず警告ログを出して次のステップへ進む
- 各ステップの後に自動で50ms待機する(`run_scenario.py`の`STEP_DELAY`)。それ以上待ちたい場合のみ`wait`ステップを追加する

## ループ

繰り返しには用途の異なる2種類の方法がある:

- **`repeat`**(アクション、上表参照): *別の*シナリオファイルを参照し、その`steps`を`count`回実行する。単体でも呼べる・複数箇所から`call_scenario`で呼ばれるような再利用可能なサブシナリオに向く。
- **インラインループ**(`loop`・`loop_count`ステップフィールド、Web UIが設定): *同じファイル内*の連続したステップを別ファイルを用意せず`loop_count`回繰り返す。Web UIでは連続したノードを選択して「ループ化」を選ぶ(グループ化と同じ操作感だが、実行時に実際に繰り返される点が異なる)。ループのラベルをダブルクリックすると名前・回数の変更や、テーブル駆動ループへの切り替え(下記参照)ができる。

```yaml
- action: set_variable
  name: i
  value: "1"

- action: wait
  ms: 500
  loop: myLoop
  loop_count: 5

- action: click_image
  images:
    - images/foo/button.png
  loop: myLoop
  loop_count: 5
```

同じ`loop`ラベルを持つステップは連続していて、かつ同じ`loop_count`・`loop_table`である必要がある — 連続していない、一致しない、または両方/どちらも設定されていない場合はバリデーションでエラーになる。

### テーブルに対するループ

固定の`loop_count`の代わりに、あらかじめ`load_table`ステップで読み込んだテーブルの行ごとにループを実行することもできる。`loop_table: <name>`を指定する(ループの各ステップは`loop_count`・`loop_table`のどちらか一方だけを設定し、両方は設定しない)。各イテレーションでは、ループ本体を実行する前に、その行の各列の値が列名と同じ名前の変数として設定される(例: 列「顧客コード」は`{{顧客コード}}`になる) — `set_variable`と同じ仕組みで、値の出どころが行データになっているだけ:

```yaml
- action: load_table
  path: C:\data\customers.xlsx
  name: customers

- action: type_text
  text: "{{顧客コード}}"
  loop: sendToEach
  loop_table: customers

- action: click_image
  images:
    - images/send_button.png
  loop: sendToEach
  loop_table: customers
```

Web UIでは、実行パネルの「取り込んだデータ」タブにある「取り込み」ボタンからファイルを選ぶと、内部用の`load_table`ステップが自動追加または更新され（キャンバスには表示されず）、全行をチェックした状態でプレビュー表示される。同時にシナリオの「開始」から「終了」までがテーブルループになる。実行しない行のチェックを外すと、その0始まりの行番号が`selected_rows`として保存され、未チェック行はスキップされる。取り込んだ列名は変数一覧や変数名のドロップダウンにも表示され、選択された各行の値がその反復の変数になる。

アクセシビリティのため、パレットのアクションをクリックすると、選択中のステップの直後に追加できます（ステップ未選択時は`終了`の前に追加）。ドラッグ操作も引き続き使えます。パレット項目、アクションノード、開始・終了ノードはキーボードでフォーカスでき、EnterまたはSpaceで操作できます。ノードにフォーカスした状態で矢印キーを押すと、その方向に最も近いノードへフォーカスを移動できます。`Alt+矢印`では配置を微調整でき、`Shift+Alt+矢印`では大きく移動します。ノード間の接続は終点が分かる矢印で表示されます。左右のパネル境界をドラッグすると幅を変更でき、境界にフォーカスして左右矢印キーでも調整できます。キャンバスの凡例では通常・実行中・条件分岐の色の意味を確認できます。

アクションを削除する際は確認を表示し、可能な場合は削除したアクションの前後を自動的に接続します。シナリオの境界となる`開始`と`終了`は削除できません。

参照先のテーブルが一度も読み込まれていない場合(`load_table`ステップが無い・失敗した、またはテーブル名の誤り)、ループは警告をログに出したうえで0回のイテレーションで終わる — 成功した空実行のように見えて実は失敗している、という事態を避けるため、この警告は目立つように出す。

## 分岐

`if`・`else`・`endif`は単なる通常の連続したステップで、他のアクションと同様に順番につなげる。Web UIには専用の操作もある: 連続したノードを選択して「if分岐化」を選ぶ(グループ化・ループ化と同じ操作感)と、新しい`if`・`else`・`endif`で選択範囲を囲む — オレンジ色の枠が表示され、「true」「false」の2領域に分かれる。上側の領域にアクションをドロップするとtrue側へ、下側の領域にドロップするとfalse側へ追加できる。`if`ノード自体をパラメータパネルで選択して`variable`・`equals`を設定できる。`else`のない既存シナリオも引き続き利用でき、枠のラベルを右クリックすると不足している`else`を追加できる。同じく右クリックから「if分岐を解除」でラッパーだけを外せる(中のステップは残り、分岐だけが解除される):

```yaml
- action: set_variable
  name: status
  value: ok

- action: if
  variable: status
  equals: ok

- action: click_image           # status == "ok" の場合のみ実行
  images:
    - images/foo/ok_button.png

- action: else

- action: click_image           # status != "ok" の場合のみ実行
  images:
    - images/foo/retry_button.png

- action: endif

- action: wait                  # if/elseブロックの後、常に実行される
  ms: 500
```

- 条件は**文字列として**比較される: `equals`は変数の値と`str(...) == str(...)`で比較されるため、`equals: "5"`は数値`5`が入った変数ともマッチする。未設定の変数は`""`として扱われる。
- `equals`を省略すると、変数が空でない値に設定されているかどうかだけをチェックする(truthyチェック)。例: 成功時のみ`result`を設定するステップの後に`if: variable: result`。
- `variable`の代わりに`last_step: warned`を指定すると、直前のアクションが警告を出しながら継続した場合(画像が見つからない、Webhookに失敗した等)にブランチを実行できる。`last_step: ok`は直前のアクションが警告を出さずに終わった場合に実行する。`variable`と`last_step`は同時に指定できない。
- `else`は省略可能 — `else`のない`if`は条件が偽の場合何もしない。
- `if`・`else`・`endif`ブロックはネストできる。バリデータはネストの深さで各`if`とその`else`・`endif`を対応付け、孤立した`else`・`endif`、同じ`if`に対する重複した`else`、`endif`のない`if`をエラーとして検出する。
- `last_step`はシナリオを継続できたアクションの結果だけを表す。致命的な例外が発生した場合は、後続の`if`に到達する前に実行が中断される。

## バリデーション

## アクション結果

すべてのアクションは、成功・既知の問題を警告して継続・予期しない例外や検証エラーで失敗停止、という3状態の契約を持つ。画像検索、ウィンドウアクティベーション、ファイル・ネットワーク操作、表の読み込み、Excelの読み書きでは想定される運用上の問題を警告として継続する。`send_webhook`はデフォルトで警告継続し、`on_error: stop`を選ぶと停止する。エディタ連携用に、`/api/actions`レスポンスの`outcomes`でもこの契約を取得できる。

`run_scenario.py`は最初のステップを実行する前に、シナリオ全体を検証する。`call_scenario`で参照される全てのファイルも再帰的に検証対象になる。これにより、RPAが画面操作を始める前の段階で書き方のミスに気付ける(実行途中で発覚するのではなく)。

- **エラー**(実行前に中断され、見つかった問題を最初の1件だけでなく全て列挙する): 未知の`action`、必須パラメータの不足、認識されないパラメータ(例: `retry`のつもりで`retries`と書くようなタイポ)、`images`・`keys`がリストになっていない、`offset`が`[x, y]`の2要素リストになっていない、`position`が認識されない値、存在しない`call_scenario`・`repeat`の参照先、`call_scenario`・`repeat`の循環参照、`repeat`の`count`が正の整数でない、`send_webhook`の`payload`がマッピングでもJSON文字列でもない、`loop`ステップが連続していない・`loop_count`が一致しない、`loop_count`が正の整数でない、孤立した`else`・`endif`、同じ`if`に対する重複した`else`、`if`に`variable`/`last_step`のどちらも指定しない・両方指定する、`last_step`の値が不正、`endif`のない`if`
- **警告**(ログに出力されるが実行は継続する): `scenarios/`配下に存在しない`images`のファイル

`note`・`group`・`title`はどのステップにも付けられる予約済みのメタキーで、ランナーはこれらを無視する(そのため「未知のパラメータ」エラーの対象にはならない)。`note`は自由記述のメモ、`group`・`title`はWeb UIのグループ化機能・ステップごとのタイトル機能が設定するもの。`loop`・`loop_count`・`loop_table`も同様に予約済みだが、他の3つと違い実行に実際に影響する — 詳細は上記のループの項を参照。
