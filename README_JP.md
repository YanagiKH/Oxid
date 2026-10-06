<p align="center">
  <img width="216" height="200" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</p>

<h1 align="center">Oxid</h1>

<p align="center">
  <strong>短いコードでスクリプトや自動化ツールを書き、ほかの言語とつなぐ。</strong>
</p>

<p align="center">
  <a href="https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml"><img alt="Repository CI" src="https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="https://github.com/YanagiKH/Oxid/releases"><img alt="Release" src="https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases" /></a>
  <a href="LICENSE"><img alt="License: MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README_ZH.md">繁體中文</a> · <a href="README_JP.md">日本語</a>
</p>

<p align="center">
  <code>.ox</code> ファイルを <code>oxid</code> で実行し、トップレベルのインポートを単一の <code>.oxb</code> にまとめられます。<br />
  インタープリターとプロジェクト用ツールを一つの実行ファイルに同梱。Oxid プログラムの実行に Rust のインストールは不要です。
</p>

<p align="center"><strong>0.9 · 実験段階 · Rust 製インタープリター</strong></p>

```oxid
fun double(n) => n * 2;

fun main() {
    for n in [1, 2, 3] {
        say n |> double;
    }
}
```

`double.ox` として保存し、`oxid run double.ox` を実行すると、`2`、`4`、`6` が一行ずつ表示されます。`fun`、`var`、`say` に加え、`fn`、`let`、`print` も使えます。

## 試してみる

[Oxid をインストール](docs/INSTALLATION.md)して、プロジェクトを作成します。

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid run .oxid/bin/hello.oxb
oxid test
```

どちらの実行コマンドも `Hello from Oxid` を表示します。ビルドで生成されるのはシリアライズされた AST で、実行には互換性のある Oxid ランタイムが必要です。

[クイックスタート](docs/QUICKSTART.md) · [構文](docs/SYNTAX.md) · [コマンド一覧](docs/COMMANDS.md)

## 今できること

Oxid 0.9 では次の機能を利用できます。各ガイドで現在の動作と制限を説明しています。

### 構文とデータ

- **短い表記と慣れた表記：** `fun` / `fn`、`var` / `let`、`say` / `print` などの別名を同じプログラムで併用
- **関数と制御構文：** 式で定義する関数、`if` / `when`、`while` / `loop`、`for … in`、`break`、`continue`、`|>` パイプライン
- **組み込みの値：** 数値、文字列、真偽値、null、配列、プロパティや文字列キーでアクセスできるレコード
- **テキストと JSON：** 文字列の分割・結合・置換、JSON の解析とシリアライズ、一定の順序でのレコードキー出力
- **ソースマクロ：** 引数付きの一行マクロを構文解析前に展開し、前処理結果をキャッシュ
- **遅延タスク：** `async` / `work`、`await`、`spawn`、状態の確認、結果の再利用。`join_all` はタスクを順番に実行

### 実行とパッケージ管理

- **直接実行：** `.ox` ファイルの実行、REPL、プロジェクトのファイル変更時に再実行する `watch`
- **持ち運べる AST 成果物：** トップレベルのインポートを `.oxb` にまとめ、同じ表現を `.oxa` として出力。形式のバージョン、件数、チェックサムを確認
- **モジュール読み込み：** 相対インポートと依存エイリアスを解決し、成果物にもソース位置を保持。関数内のインポートには実行時もソースファイルが必要
- **依存関係の固定：** ローカルパス、完全なコミット ID で固定した HTTPS Git ソース、再帰的な依存解決、`oxid.lock`、locked / offline / frozen モード

### プロジェクト用ツール

- **ひな形の作成：** `new` / `init` と HTTP・Discord プロジェクトの生成
- **日常のチェック：** 構文チェック、構文検証ベースの `lint`、ソースの整形、プロジェクト内のテストとサンプルの実行
- **マニフェストスクリプト：** 引用符付き引数に対応した再利用可能なコマンド。シェルを介さずプロセスを起動
- **プロジェクトの確認：** `doctor`、API ドキュメント生成、起動・解析・パッケージ化・ランタイム操作のベンチマークレポート

### ファイルとネットワーク、言語間連携

- **ローカル I/O：** テキストファイル、ディレクトリ一覧、環境変数、時刻、sleep 関数
- **外部プログラム：** プロセスの起動、終了コードや標準出力の取得、プロセスアダプターによる Python・Java・Go の呼び出し
- **C/C++ とホストアダプター：** リンク済みの四つのネイティブ補助関数と、Oxid を起動する Python・Java・Go・C・C++ アダプターの生成
- **ローカル HTTP と Discord ロジック：** 再利用可能な TCP リスナー、タイムアウト付き HTTP 関数、ルーティング、Discord コマンドとインタラクションの振り分け。ゲートウェイ通信は外部で処理

[構文](docs/SYNTAX.md) · [ランタイム API](docs/API.md) · [パッケージ](docs/PACKAGES.md) · [プロジェクトツール](docs/COMMANDS.md) · [言語間連携](docs/INTEROP.md) · [HTTP / Discord](docs/WEB_AND_BOTS.md)

[サンプル](examples/)で使い方を試せます。外部アダプターには、それぞれのランタイムやツールチェーンが必要です。`.oxb` はシリアライズされた AST であり、ネイティブの機械語ではありません。実行には互換性のある Oxid ランタイムが必要です。

## 利用前に知っておくこと

Oxid 0.9 は実験や小さなツール向けです。`.oxb` にはシリアライズされた AST が入り、インタープリターで実行されます。タスクは遅延実行され、join 時に順番に処理されます。ネットワーク機能には並行スケジューラーや組み込み TLS はありません。互換性のために残している `bootstrap` と `self-host` は、成果物のシリアライズ往復を検証するコマンドで、コンパイラ自身を再ビルドするものではありません。

長期的には、静的チェックとネイティブコンパイルを備えた言語を目指しています。任意選択の[実験的 LLVM プレビュー](spec/native-preview.md)は、制限付きで再帰のない bool／unit／i32 の部分集合を Linux x86_64 実行ファイルにコンパイルできます。完全な静的コアの意味論、Rust 互換性、ネイティブ AI 学習は未提供です。[ロードマップ](docs/ROADMAP.md)と[実装状況](docs/architecture/current-baseline.md)を参照してください。

任意選択の[所有権基盤拡張](spec/typed-preview.md#nominal-owned-structs-and-call-only-borrowing)は `--edition typed-preview` で、スカラーのフィールドだけを持つムーブ専用 struct、値全体の移動・置換、関数呼び出しに限定した明示的な借用をサポートします。[Batch サンプル](fixtures/owned_source/batch.ox)はループと所有値を返す補助関数を組み合わせ、816 を返します。実際の検証範囲と制限は[ソース検証記録](docs/architecture/owned-source-validation.md)を参照してください。実行・ネイティブコンパイルの `main` は引数を取らずスカラーを返し、ネイティブ対象は引き続き Linux x86_64、LLVM 19.1.7、O0 に限定されます。既存の動的レコードの意味論は変わりません。参照の保存、ヒープ・デストラクターの安全性、v1.0 の完成は今回の範囲外です。

実験的な[型付きプロジェクト経路](spec/typed-preview.md#bounded-typed-projects)では、宣言専用モジュール、直接インポート、可視性を使用できます。[3 ファイルの Batch サンプル](fixtures/typed-project-batch/README.md)はフィールドを非公開に保ち、ソースから導いた結果は 816 です。宣言済み子モジュールの読み込みは現在 Linux に限定され、ルートファイルだけの構文にはこの探索用ホスト制限はありません。実際の検証範囲は[プロジェクト検証記録](docs/architecture/typed-project-unit4-validation.md)を参照してください。

実験的な[固定長スカラー配列拡張](spec/typed-preview.md#fixed-scalar-arrays)は、typed-preview を明示した `check`、`run`、`compile` に、ムーブ専用 bool/i32/unit 配列、境界検査付き添字、`len()`、呼び出し中の配列全体の借用を追加します。[3 モジュールのサンプル](fixtures/typed-array-samples/README.md)は 5325 を返します。構文、対象プラットフォーム、より厳しいネイティブ制限は仕様を参照してください。既定および legacy の配列動作は変わりません。

実験的な[呼び出し限定の借用スカラースライス](spec/typed-preview.md#call-only-borrowed-scalar-slices)では、共有 `&[T]` と排他的 `&mut [T]` の補助関数が、明示的な借用・再借用を通じて長さの異なる bool/i32/unit 固定長配列全体を処理できます。[3 モジュールのスライスサンプル](fixtures/typed-slice-samples/README.md)は長さ 2、3、0 を使い、515 を返します。対象は typed-preview を明示した `check`、`run`、ネイティブ `compile` に限られ、既存の Linux 限定モジュール読み込みと、Linux x86_64・LLVM/Clang/LLD 19.1.7・O0 のネイティブ制限を維持します。範囲指定、部分スライス、所有されるサイズ不定値は未対応であり、安定性やマイルストーンの完成を意味しません。

実験的な[型付きフォーマッター](spec/typed-preview.md#single-file-formatting)は固定長配列の構文と `oxid fmt --edition typed-preview input.ox`（整形済みソース全体を標準出力へ）と `--check`（整形が必要なら終了コード 1）をサポートします。コメントと既存の改行を保持して空白とインデントを整え、モジュールの読み込みやファイルへの書き込みは行いません。既定の従来フォーマット動作は変更されません。

信頼できるプログラムだけを実行し、依存関係を確認してください。生成された C/C++ プロセスアダプターには、信頼できるパスだけを渡してください。脆弱性の非公開報告については[セキュリティポリシー](SECURITY.md)を参照してください。

## ソースからビルド

安定版 Rust と C/C++ コンパイラが必要です。以下は Unix 系シェルで実行します。

```bash
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
cargo build --release --locked
./target/release/oxid run examples/hello.ox
```

Windows、リリースアーカイブ、Cargo インストール、Docker については[インストールガイド](docs/INSTALLATION.md)を参照してください。

## もっと知る・参加する

[ドキュメント](docs/README.md) · [アーキテクチャ](docs/ARCHITECTURE.md) · [貢献ガイド](CONTRIBUTING.md) · [Issues](https://github.com/YanagiKH/Oxid/issues)

図で見る：[クイックスタート](docs/assets/quickstart.svg)、[実行の流れ](docs/assets/architecture.svg)、[言語間連携](docs/assets/interop.svg)、[Web / Discord](docs/assets/web-discord.svg)。

最小の実行例を添えたバグ報告、エラーメッセージの改善、テストやドキュメントの修正を歓迎します。プルリクエストを開く前に、[貢献ガイド](CONTRIBUTING.md)のチェックを実行してください。

[MIT](LICENSE) または [Apache-2.0](LICENSE-APACHE) ライセンスで利用できます。

実験的な[配列フィールドのスライス借用](rfcs/0022-projected-array-slices.md)では、`bump(&mut batch.samples)` のように固定長スカラー配列フィールドをスライス補助関数に渡せます。入れ子のフィールドパスとレコード全体の参照からの明示的な再借用でも、所有者全体の競合規則、可視性、呼び出し限定の寿命を維持します。リソース上限とネイティブ対象は変更しません。

実験的な[上限付き列挙型と消費型マッチ](spec/typed-preview.md#bounded-nominal-enums-and-consuming-match)は、データなし、または bool/i32/unit を一つ持つバリアントからなる公称型のムーブ専用値と、名前付き所有者を網羅する match 文を追加します。typed-preview を明示した check/run/ネイティブ compile と整形がこの契約をサポートします。[2 ファイルのスキャナー](tests/fixtures/bounded_enum_scanner/main.ox)は 115 を返します。列挙型の借用、集約型ペイロード、match 式は対象外です。現在のソースの検証と正確なコミットに対するホスト型 CI は、別途完了が必要です。セルフホスティングの達成を意味しません。

実験的な[上限付き標準入力](spec/typed-preview.md#bounded-stdin-input)は、別名も使える個別の `std::io::read_stdin` と `std::io::ReadStatus` インポートだけを追加します。既存の排他的 i32 スライスに生バイトを読み込み、`Eof(n)`、余分な読み取りを行わない `Full`、または書き込み先を変更しない `IoError` を返します。消費済みの入力は巻き戻せません。Linux x86_64 では、[128 バイトの式処理プログラム](fixtures/typed-expression-samples/README.md#bounded-stdin-entry)がローカルの 28 ケースの参照／ネイティブ検証で、同じ未変更の ELF から 39 と 63 を返しました。現在のソース全体の検証と正確なコミットに対するホスト型 CI は別の関門です。一般的な `std`、文字列、他の入力実行環境、セルフホスティングは今回の対象外です。

実験的な[上限付き標準出力とプロセス入口](spec/typed-preview.md#bounded-stdout-and-process-entry)は、既存の共有 i32 バイトスライスを受け取る `write_stdout` と、明示的な `--entry-mode=process` を追加します。0..255 をそのまま終了状態にし、標準出力にスカラーや JSON の結果を追加しません。[永続化スタックコンポーネント](fixtures/typed-expression-samples/README.md)は 80 バイトの OXS1 ファイルを生成し、独立した Oxid ローダーと外部デコーダーが検証します。同じ生成器 ELF で 39 と 63 の入力を処理できます。実行対象は Linux x86_64 です。現在のソースの検証とホスト型 CI は別の関門であり、Windows の Process Run は現在、出力せず状態 74 で終了します。既定モードは変わりません。

Oxid で記述した独立の[上限付き typed-preview 字句解析コンポーネント](fixtures/typed-lexer-samples/README.md)は、最大 128 バイトの ASCII ソースについてトークン、空白・コメント、バイト範囲を保持します。既存の Rust 字句解析器と比較し、同じネイティブ ELF で実行します。本番コンパイラの実装切り替えと Unicode 対応は対象外です。
