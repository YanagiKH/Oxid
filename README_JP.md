[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)

# Oxid

Oxid は、Rust のブートストラップランタイムと、Oxid で記述された機能領域（その範囲は拡大中）を基盤として構築された実験的な言語プロトタイプです。

その目的は、単なる薄いラッパーにとどまることではありません。このプロジェクトは、標準ライブラリモジュール、サンプル、ツールスクリプト、パッケージのワークフロー、プロジェクトのドキュメントなど、日常的な開発作業が徐々に Oxid のソースファイルへと移行していくように構成されています。

## 現在利用可能な機能

- `oxid run <file.ox>`: スクリプトを実行
- `oxid script <name> [args...]`: マニフェストスクリプトを実行
- `oxid repl`: 対話型REPL
- `oxid check <file.ox>`: 構文チェック
- `oxid new <project-name>` / `oxid init <project-name>`: プロジェクトのスケルトンを作成
- `oxid add <name> <path-or-target>`: 依存関係エントリを追加
- `oxid watch <file.ox>`: ファイルを監視し、変更時に再実行
- `oxid build`: プロジェクトを検証
- `oxid clean`: ビルドキャッシュをクリア
- `oxid fmt [path]`: Oxidソースファイルをフォーマット
- `oxid test`: スモークテストとサンプルを実行
- `oxid doctor`: プロジェクトの状態を確認する
- `oxid doc`: APIドキュメントを生成する

## 言語機能

- `let` / `const`
- `print` / `if` / `while` / `fn` / `async fn` / `await` / `return` / `use`
- 配列、インデックス、インデックス付き代入
- `len` / `push` / `pop` / `range` / `str` / `sleep`
- `c_len` / `c_hash` / `cpp_len` / `cpp_hash`
- モジュールキャッシュ、プリプロセスキャッシュ、再帰的読み込み
- 1行の `macro` によるプリプロセス展開
- パッケージマニフェストのサポート (`oxid.toml` 内のスクリプト / 依存関係 / 機能)

## リポジトリの構成

- `src/` には、Rust ブートストラップランタイムとエントリスクリプトが含まれています
- `stdlib/` には、Oxid 標準ライブラリモジュールが含まれています
- `examples/` には、実行可能な Oxid のサンプルが含まれています
- `tools/` には、Oxid ツールのプレビュー版が含まれています
- `packages/demo/` には、ユーザー向けのパッケージ構成が含まれています
- `docs/` には、ワークフローおよびアーキテクチャに関する説明が含まれています
- `tests/` には、スモークテストが含まれています

## 推奨される初回実行手順

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/stdlib_smoke.ox
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## パッケージファーストのワークフロー

想定されるユーザー体験は以下の通りです：

1. `oxid new` でプロジェクトを作成します。
2. 再利用可能なモジュールを `src/` および `stdlib/` に配置します。
3. `oxid.toml` でスクリプトを定義します。
4. 繰り返し実行するタスクには `oxid script <name>` を使用します。
5. サンプルやスモークテストはパッケージと一緒に保管します。
6. リリース前に `oxid build`、`oxid test`、`oxid doctor`、および `oxid doc` を実行する。

詳細な手順については、`docs/QUICKSTART.md` および `docs/PACKAGE_WORKFLOW.md` を参照してください。