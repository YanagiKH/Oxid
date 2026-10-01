<div align="center">
  <img width="108" height="100" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</div>

# Oxid

**短いコードでスクリプトや自動化ツールを書き、ほかの言語とつなぐ。**

[![Repository CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

`.ox` ファイルを `oxid` で実行するか、単一の `.oxb` ファイルにまとめられます。リリース版の実行ファイルにはインタープリターとプロジェクト用ツールが含まれ、Oxid プログラムの実行に Rust のインストールは不要です。

**現在のバージョン：0.9、実験段階。** 実行環境は Rust 製のインタープリターです。ネイティブコンパイル、静的な所有権チェック、コンパイラのセルフホスティングは今後の開発項目です。

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

- 関数、ループ、パイプライン、配列、レコード、JSON を使ったスクリプト作成
- ファイルの読み書きや外部プロセスの起動、Python・Java・Go とのプロセス連携
- モジュールを単一の `.oxb` にまとめ、ローカル依存や Git の特定コミットを固定
- ローカル HTTP サービスや Discord インタラクション処理の試作
- Python・Java・Go・C・C++ から Oxid を呼び出すアダプターの生成

[サンプル](examples/)、[ランタイム API](docs/API.md)、[言語間連携ガイド](docs/INTEROP.md)から始められます。外部アダプターには、それぞれのランタイムやツールチェーンが必要です。

## 利用前に知っておくこと

Oxid 0.9 は実験や小さなツール向けです。`.oxb` にはシリアライズされた AST が入り、インタープリターで実行されます。タスクは遅延実行され、join 時に順番に処理されます。ネットワーク機能には並行スケジューラーや組み込み TLS はありません。互換性のために残している `bootstrap` と `self-host` は、成果物のシリアライズ往復を検証するコマンドで、コンパイラ自身を再ビルドするものではありません。

長期的には、静的チェックとネイティブコンパイルを備えた言語を目指しています。これらの機能、Rust 互換性、ネイティブ AI 学習は 0.9 にはありません。[ロードマップ](docs/ROADMAP.md)と[実装状況](docs/architecture/current-baseline.md)を参照してください。

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
