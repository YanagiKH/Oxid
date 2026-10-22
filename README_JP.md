<img align="right" width="96" alt="Oxid ロゴ" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

# Oxid

**Rust に着想を得た、スクリプトとコマンドラインツールのためのプログラミング言語。**

`.ox` ファイルを書いて、`oxid` で実行できます。公開中の実験版 0.9 は、
インタープリター、フォーマッター、テスト実行、プロジェクト管理を一つの実行ファイルにまとめています。

[インストール](docs/INSTALLATION.md) · [クイックスタート](docs/QUICKSTART.md) · [ドキュメント](docs/README.md)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

## 試してみる

[Releases](https://github.com/YanagiKH/Oxid/releases) から利用するプラットフォームのアーカイブを取得し、
展開した `oxid` または `oxid.exe` を `PATH` に追加してください。公開バイナリの利用に Rust は不要です。
[インストールガイド](docs/INSTALLATION.md)には、インストーラーとソースからのビルド手順もあります。

次のプログラムを `double.ox` として保存します。

```oxid
fn main() {
    for n in [1, 2, 3] {
        print n * 2;
    }
}
```

実行します。

```sh
oxid run double.ox
```

`2`、`4`、`6` が一行ずつ表示されます。

## プロジェクトを作る

```sh
oxid new hello
cd hello
oxid run src/main.ox
oxid test
oxid build
oxid run .oxid/bin/hello.oxb
```

初期プログラムは `Hello from Oxid` と表示します。`src/main.ox` を編集して内容を変更できます。
`oxid build` は `.oxid/bin/hello.oxb` を生成します。このファイルは互換性のある Oxid で実行できます。
データ処理やプロジェクトの操作は、[クイックスタート](docs/QUICKSTART.md)を参照してください。

## 開発版のネイティブプロジェクト

開発中のソースでは、明示的な型と所有権の検査を備えたプロジェクトから、
ネイティブ実行ファイルを生成できます。この実験的な機能は、
**公開済みの 0.9 バイナリには含まれていません**。

ネイティブビルドには現在、**Linux x86_64**、**LLVM/Clang/LLD 19.1.7**、
システムの C 開発ツールが必要です。実行ステップ数と使用できるリソースには上限があります。

作成、チェック、テスト、ビルドの手順は、[ネイティブプロジェクトガイド](docs/TYPED_PROJECTS.md)にあります。
ローカル依存関係、ワークスペース、ソースパッケージの使い方も説明しています。
[バイト数を数えるアプリケーション](apps/byte-count/README.md)は、複数パッケージで構成する実例です。

## ガイドとサンプル

- [言語の構文](docs/SYNTAX.md)と[組み込み関数](docs/API.md)
- [モジュール](docs/MODULES.md)と[スクリプトの依存関係](docs/PACKAGES.md)
- [コマンドリファレンス](docs/COMMANDS.md)：整形、テスト、プロジェクト管理
- [他の言語の呼び出し](docs/INTEROP.md)
- [サンプルプログラム](examples/)

## 開発に参加する

Oxid 本体をビルドするには、[ソースからのインストール手順](docs/INSTALLATION.md#build-a-checkout)を参照してください。
変更の提案や不具合の報告については、[貢献ガイド](CONTRIBUTING.md)と
[Issues](https://github.com/YanagiKH/Oxid/issues)をご覧ください。

[MIT](LICENSE) または [Apache-2.0](LICENSE-APACHE) ライセンスで利用できます。
