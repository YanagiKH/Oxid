<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

# Oxid

Oxid 是一個以 Rust 啟動運行時為核心，並逐步擴展由 Oxid 編寫的程式碼範圍所建構的實驗性語言原型。

其目標並非僅止於作為一個輕量級的封裝層。該專案的組織架構旨在讓日常開發工作逐步轉移至 Oxid 原始碼檔案中，包括標準函式庫模組、範例、工具腳本、套件工作流程以及專案文件。

## 現有功能

- `oxid run <file.ox>`：執行腳本
- `oxid script <name> [args...]`：執行清單腳本
- `oxid repl`：互動式 REPL
- `oxid check <file.ox>`：語法檢查
- `oxid new <專案名稱>` / `oxid init <專案名稱>`：建立專案骨架
- `oxid add <名稱> <路徑或目標>`：新增依賴項
- `oxid watch <file.ox>`：監控檔案並重新執行
- `oxid build`：驗證專案
- `oxid clean`：清除建置快取
- `oxid fmt [路徑]`：格式化 Oxid 原始碼檔案
- `oxid test`：執行煙霧測試與範例
- `oxid doctor`：驗證專案健康狀態
- `oxid doc`：產生 API 文件

## 語言特性

- `let` / `const`
- `print` / `if` / `while` / `fn` / `async fn` / `await` / `return` / `use`
- 陣列、索引及索引賦值
- `len` / `push` / `pop` / `range` / `str` / `sleep`
- `c_len` / `c_hash` / `cpp_len` / `cpp_hash`
- 模組快取、預處理快取與遞迴載入
- 單行 `macro` 預處理展開
- 套件清單支援（`oxid.toml` 腳本／依賴項／功能）

## 儲存庫結構

- `src/` 包含 Rust 啟動運行時與入口腳本
- `stdlib/` 包含 Oxid 標準函式庫模組
- `examples/` 包含可執行的 Oxid 範例
- `tools/` 包含 Oxid 工具預覽版
- `packages/demo/` 包含面向使用者的套件佈局
- `docs/` 包含工作流程與架構說明
- `tests/` 包含煙霧測試

## 建議的首次執行步驟

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/stdlib_smoke.ox
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## 以套件為先的工作流程

預期的使用者體驗如下：

1. 使用 `oxid new` 建立專案。
2. 將可重複使用的模組放置於 `src/` 和 `stdlib/` 目錄中。
3. 在 `oxid.toml` 中定義腳本。
4. 針對可重複執行的任務，使用 `oxid script <名稱>`。
5. 將範例程式碼和煙霧測試與套件一併存放。
6. 發佈前執行 `oxid build`、`oxid test`、`oxid doctor` 及 `oxid doc`。

如需完整操作指南，請參閱 `docs/QUICKSTART.md` 及 `docs/PACKAGE_WORKFLOW.md`。