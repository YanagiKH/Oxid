<p align="center">
  <img width="216" height="200" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</p>

<h1 align="center">Oxid</h1>

<p align="center">
  <strong>用簡潔語法撰寫腳本、自動化工具，串接不同語言。</strong>
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
  寫一個 <code>.ox</code> 檔案，就能用 <code>oxid</code> 執行，也能將頂層匯入打包成單一 <code>.oxb</code> 檔案。<br />
  一個執行檔內含直譯器和專案工具；執行 Oxid 程式不需要安裝 Rust。
</p>

<p align="center"><strong>0.9 · 實驗階段 · 以 Rust 撰寫的直譯器</strong></p>

```oxid
fun double(n) => n * 2;

fun main() {
    for n in [1, 2, 3] {
        say n |> double;
    }
}
```

存成 `double.ox`，執行 `oxid run double.ox`，就會逐行印出 `2`、`4`、`6`。除了 `fun`、`var`、`say`，也可以使用 `fn`、`let`、`print`。

## 馬上試試

先[安裝 Oxid](docs/INSTALLATION.md)，再建立專案：

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid run .oxid/bin/hello.oxb
oxid test
```

兩個執行指令都會印出 `Hello from Oxid`。建置產生的是序列化 AST 檔案，需要相容的 Oxid 執行環境才能執行。

[快速入門](docs/QUICKSTART.md) · [語法](docs/SYNTAX.md) · [指令參考](docs/COMMANDS.md)

## 現在可以做什麼

Oxid 0.9 已提供以下功能。各項指南另有目前行為與限制的說明。

### 語法與資料

- **簡潔或熟悉的寫法：** `fun` / `fn`、`var` / `let`、`say` / `print` 等別名可以在同一支程式混用
- **函式與流程控制：** 單一運算式函式、`if` / `when`、`while` / `loop`、`for … in`、`break`、`continue` 與 `|>` 管線
- **內建資料型別：** 數值、字串、布林值、null、陣列，以及支援屬性或字串鍵存取的 record
- **文字與 JSON：** 分割、合併、取代文字，解析與序列化 JSON，並固定 record 的鍵排序
- **原始碼巨集：** 在解析前展開單行、可帶參數的巨集，並快取預處理結果
- **延遲執行的 task：** `async` / `work`、`await`、`spawn`、狀態查詢與結果快取；`join_all` 依序執行 task

### 執行、打包與相依套件

- **直接執行：** 執行 `.ox` 檔案、使用 REPL，或透過 `watch` 在專案檔案變更後重新執行
- **可攜 AST 產物：** 將頂層匯入打包為 `.oxb`，以 `.oxa` 輸出相同表示，並檢視格式版本、數量與檢查碼
- **模組載入：** 解析相對路徑與相依套件別名，在產物中保留原始碼位置；函式內的匯入仍需在執行時取得原始檔
- **相依套件鎖定：** 本機路徑、以完整 commit 鎖定的 HTTPS Git 來源、遞迴解析、`oxid.lock`，以及 locked / offline / frozen 模式

### 專案工具

- **起始專案：** `new` / `init`，以及 HTTP、Discord 專案範本
- **日常檢查：** 語法檢查、以語法檢查為基礎的 `lint`、原始碼格式化，以及執行專案測試與範例
- **Manifest 腳本：** 可重複使用的指令，支援引號參數，直接啟動程序而不經 shell
- **專案檢視：** `doctor`、API 文件產生，以及啟動、解析、打包與執行環境操作的 benchmark 報告

### 檔案、網路與跨語言串接

- **本機 I/O：** 文字檔案、目錄列表、環境變數、時間查詢與 sleep 函式
- **外部程式：** 啟動程序、取得結束碼或標準輸出，透過程序轉接器呼叫 Python、Java 或 Go
- **C/C++ 與主程式轉接器：** 四個已連結的原生輔助函式，以及能啟動 Oxid 的 Python、Java、Go、C、C++ 轉接程式產生器
- **本機 HTTP 與 Discord 邏輯：** 可重複使用的 TCP listener、有逾時限制的 HTTP 函式、路由，以及 Discord 指令與互動分派；gateway 傳輸仍由外部處理

[語法](docs/SYNTAX.md) · [執行環境 API](docs/API.md) · [套件](docs/PACKAGES.md) · [專案工具](docs/COMMANDS.md) · [跨語言串接](docs/INTEROP.md) · [HTTP / Discord](docs/WEB_AND_BOTS.md)

可先執行[範例](examples/)了解用法。外部轉接器仍需要對應的執行環境或工具鏈。`.oxb` 需要相容的 Oxid 執行環境；內容是序列化 AST，並非原生機器碼。

## 使用前先了解

Oxid 0.9 適合實驗與小型工具。`.oxb` 內含序列化 AST，由直譯器執行。Task 採延遲執行，join 時依序執行；網路功能尚無並行排程器或內建 TLS。沿用的 `bootstrap`、`self-host` 指令驗證的是產物序列化往返，並非編譯器重建自身。

長期方向是具備靜態檢查與原生編譯的語言。可選的[實驗性 LLVM 預覽](spec/native-preview.md)已能將受限、非遞迴的 bool／unit／i32 子集編譯成 Linux x86_64 執行檔。完整靜態核心語意、Rust 相容性與原生 AI 訓練仍未提供。請參考[路線圖](docs/ROADMAP.md)與[實作現況](docs/architecture/current-baseline.md)。

可選的[所有權基礎擴充](spec/typed-preview.md#nominal-owned-structs-and-call-only-borrowing)適用於 `--edition typed-preview`，支援僅含純量欄位、只能移動的 struct、整體移動與替換，以及僅限函式呼叫的顯式借用。[Batch 範例](fixtures/owned_source/batch.ox)結合迴圈與回傳擁有值的輔助函式，結果為 816；實際證據與限制見[原始碼驗證紀錄](docs/architecture/owned-source-validation.md)。執行與原生編譯的 `main` 仍須無參數並回傳純量；原生範圍仍限 Linux x86_64、LLVM 19.1.7、O0。既有動態 record 語意不變；儲存參照、堆積配置與析構安全，以及 v1.0 完成，均不在本次範圍內。

只執行可信任的程式，並審查相依套件。產生的 C/C++ 程序轉接器只應接收可信任路徑。私下回報漏洞的方式見[安全政策](SECURITY.md)。

## 從原始碼建置

需要穩定版 Rust 與 C/C++ 編譯器。以下指令適用於 Unix 類 shell：

```bash
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
cargo build --release --locked
./target/release/oxid run examples/hello.ox
```

Windows、發行版壓縮檔、Cargo 安裝與 Docker 用法，請見[安裝指南](docs/INSTALLATION.md)。

## 深入了解與參與

[文件](docs/README.md) · [架構](docs/ARCHITECTURE.md) · [貢獻指南](CONTRIBUTING.md) · [問題回報](https://github.com/YanagiKH/Oxid/issues)

圖解：[快速入門](docs/assets/quickstart.svg)、[執行流程](docs/assets/architecture.svg)、[跨語言串接](docs/assets/interop.svg)、[Web / Discord](docs/assets/web-discord.svg)。

歡迎提供附最小可執行範例的 bug 回報、改善錯誤訊息、補測試或修正文件。送出 pull request 前，請依[貢獻指南](CONTRIBUTING.md)執行相關檢查。

採用 [MIT](LICENSE) 或 [Apache-2.0](LICENSE-APACHE) 授權。
