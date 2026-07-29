<div align="center">
  <picture>
    <img width="304" height="281.5" alt="Oxid" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
  </picture>

[English](README.md) / [繁體中文](README_ZH.md) / [日本語](README_JP.md)
</div>

# Oxid

Oxid 是 Rust 的衍生語言，並具備專屬的自宿主式語言工具鏈。其目標是打造一種比 Rust 更快、更簡潔且更易於閱讀的語言，並擁有專屬的語法、模組、命令列工作流程以及診斷模型。

其目標並非僅止於作為一個輕量級的封裝層。該專案的組織架構旨在讓日常開發工作逐步轉移至 Oxid 原始碼檔案中，包括標準函式庫模組、範例、工具腳本、套件工作流程以及專案文件。

## 現有功能

- `oxid bootstrap`：驗證啟動路徑
- `oxid frontend`：預覽詞法分析／語法分析／抽象語法樹／恢復機制／模組／語法流程
- `oxid diagnose`：渲染與原始碼關聯的診斷資訊
- `oxid lint`：預覽風格與結構規則
- `oxid emit`：預覽發行結果
- `oxid module`：預覽模組解析
- `oxid syntax`：預覽簡化語法形式
- `oxid interop`：預覽 C/C++/Java/Python 橋接支援範圍
- `oxid bridge`：預覽雙向橋接輔助函式
- `oxid self-host`：彙總從 Rust 遷移的途徑
- `oxid run <file.ox>`：執行腳本
- `oxid script <name> [args...]`：執行清單腳本
- `oxid repl`：互動式 REPL
- `oxid check <file.ox>`：語法檢查
- `oxid new <專案名稱>` / `oxid init <專案名稱>`：建立專案骨架
- `oxid add <名稱> <路徑或目標>`：新增依賴項條目
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

- `src/` 包含 Rust 啟動運行時及入口腳本
- `stdlib/` 包含 Oxid 標準函式庫模組及工具工作流程
- `examples/` 包含可執行的 Oxid 範例
- `tools/` 包含 Oxid 工作流程預覽
- `packages/demo/` 包含面向使用者的套件結構
- `docs/` 包含工作流程、編譯器、語法、診斷及互通性說明
- `tests/` 包含煙霧測試

## 建議的首次執行步驟

```bash
oxid bootstrap
oxid frontend
oxid diagnose
oxid lint
oxid emit
oxid module
oxid syntax
oxid interop
oxid bridge
oxid self-host
```

## 以套件為先的工作流程

1. 使用 `oxid new` 建立專案。
2. 在新腳本中導入 `stdlib/prelude.ox`。
3. 將可重複使用的輔助函式置於 `stdlib/` 目錄下。
4. 將應用程式程式碼置於 `src/` 目錄下。
5. 將編譯器及工作流程預覽程式置於 `tools/` 目錄下。
6. 將可執行的範例置於 `examples/` 目錄下。
7. 發布前請執行 `oxid build`、`oxid test`、`oxid doctor` 及 `oxid doc`。

## 設計理念

在日常工作中，Oxid 應給人比 Rust 更簡潔的感受：

- 更少的顯式架構設定步驟
- 簡潔的模組與匯入語法
- 輕量級的命令驅動工作流程
- 包含行號、欄位、提示及恢復上下文的可讀性高的診斷訊息
- 透過 Oxid 端的橋接輔助程式，可輕鬆與 C/C++、Java 及 Python 整合
- 可重複使用的預覽模組，日後可成為一級編譯器功能

請參閱：

- `docs/SELF_HOSTING.md`
- `docs/FRONTEND.md`
- `docs/SYNTAX.md`
- `docs/DIAGNOSTICS.md`
- `docs/MODULES.md`
- `docs/INTEROP.md`
- `docs/PACKAGE_WORKFLOW.md`
- `docs/COMMANDS.md`
- `docs/ROADMAP.md`