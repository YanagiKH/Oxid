<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

# Oxid

Oxid は Rust の派生言語であり、独自のセルフホスト型言語ツールチェーンを備えています。その目標は、Rust よりも高速で、簡潔で、読みやすい言語を構築することであり、独自の構文、モジュール、コマンドラインワークフロー、および診断モデルを備えています。

その目的は、単なる薄いラッパーにとどまることではありません。このプロジェクトは、標準ライブラリモジュール、サンプル、ツールスクリプト、パッケージのワークフロー、プロジェクトのドキュメントなど、日常的な開発作業が徐々に Oxid のソースファイルへと移行していくように構成されています。

## 現在利用可能な機能

- `oxid bootstrap`: ブートストラップパスの検証
- `oxid frontend`: レキシカル解析／構文解析／AST／リカバリ／モジュール／構文フローのプレビュー
- `oxid diagnose`: ソースにリンクされた診断情報の表示
- `oxid lint`: スタイルおよび構造に関するルールのプレビュー
- `oxid emit`: エミッションのプレビュー
- `oxid module`: モジュール解決のプレビュー
- `oxid syntax`: 短縮構文形式をプレビュー
- `oxid self-host`: Rustからの移行パスを要約
- `oxid interop`：ブリッジのプレビューを追加
- `oxid bridge`：ブリッジのプレビューを追加
- `oxid run <file.ox>`: スクリプトを実行
- `oxid script <name> [args...]`: マニフェストスクリプトを実行
- `oxid repl`: 対話型REPL
- `oxid check <file.ox>`: 構文チェック
- `oxid new <プロジェクト名>` / `oxid init <プロジェクト名>`: プロジェクトのスケルトンを作成
- `oxid init <プロジェクト名>`: スタイルおよび構造ルールをプレビュー
- `oxid add <名前> <パスまたはターゲット>`: 依存関係エントリを追加
- `oxid watch <file.ox>`: ファイルを監視し、再実行する
- `oxid build`: プロジェクトを検証する
- `oxid clean`: ビルドキャッシュをクリアする
- `oxid fmt [path]`: Oxidソースファイルをフォーマットする
- `oxid test`: スモークテストとサンプルを実行する
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
- `stdlib/` には、Oxid 標準ライブラリモジュールとツールワークフローが含まれています
- `examples/` には、実行可能な Oxid のサンプルが含まれています
- `tools/` には、Oxid ワークフローのプレビューが含まれています
- `packages/demo/` には、ユーザー向けのパッケージ構成が含まれています
- `docs/` には、ワークフロー、コンパイラ、構文、診断、および相互運用に関する注意事項が含まれています
- `tests/` には、スモークテストが含まれています

## 推奨される初回実行手順

```bash
oxid bootstrap
oxid frontend
oxid diagnose
oxid lint
oxid emit
oxid module
oxid syntax
oxid interop
oxid self-host
```

## パッケージファーストのワークフロー

1. `oxid new` でプロジェクトを作成します。
2. 新規スクリプトで `stdlib/prelude.ox` をインポートします。
3. 再利用可能なヘルパー関数は `stdlib/` に配置します。
4. アプリケーションのコードは `src/` に配置します。
5. コンパイラやワークフローのプレビューは `tools/` に配置します。
6. 実行可能なサンプルは `examples/` に配置します。
7. リリース前に `oxid build`、`oxid test`、`oxid doctor`、`oxid doc` を実行します。

## 設計意図

Oxidは、日常的な作業においてRustよりも短く感じられるようにすべきです：

- 明示的な準備作業の手順が少ない
- コンパクトなモジュールおよびインポート形式
- 軽量なコマンド駆動型ワークフロー
- 行、列、ヒント、および復旧コンテキストを含む読みやすい診断情報
- Oxid側のブリッジヘルパーを通じて、C/C++、Java、Pythonとの統合が容易
- 後で第一級のコンパイラ機能となり得る、再利用可能なプレビューモジュール

参照：

- `docs/SELF_HOSTING.md`
- `docs/FRONTEND.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/MODULES.md`
- `docs/INTEROP.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/COMMANDS.md`
- `docs/ROADMAP.md`