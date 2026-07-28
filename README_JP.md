<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

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
- `docs/` には、ワークフロー、コンパイラ、および診断に関する注意事項が含まれています
- `tests/` には、スモークテストが含まれています

## 推奨される初回実行手順

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/frontend_preview.ox
cargo run -- run examples/module_resolution.ox
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## パッケージファーストのワークフロー

1. `oxid new` でプロジェクトを作成します。
2. 新規スクリプトで `stdlib/prelude.ox` をインポートします。
3. 再利用可能なヘルパー関数は `stdlib/` に配置します。
4. アプリケーションのコードは `src/` に配置します。
5. コンパイラやワークフローのプレビューは `tools/` に配置します。
6. 実行可能なサンプルは `examples/` に配置します。
7. リリース前に `oxid build`、`oxid test`、`oxid doctor`、`oxid doc` を実行します。

## セルフホスティングの方向性

現在の計画は段階的に進めることになっています：

1. Rustのブートストラップランタイムを一時的なランチャーとして維持する
2. フロントエンドのヘルパーロジックをOxidモジュールに移行する
3. 診断情報のフォーマット処理をOxidモジュールに移行する
4. モジュール解決処理をOxidモジュールに移行する
5. Oxidに高レベルの構文解析ヘルパーを追加する
6. コンパイラのワークフローを統合する機能をさらにOxidに移行する
7. Rustの使用をブートストラップ専用へと段階的に縮小する

参照：

- `docs/SELF_HOSTING.md`
- `docs/FRONTEND.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/MODULES.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/ROADMAP.md`