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
