<div align="center">
  <img width="108" height="100" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</div>

# Oxid

**用簡潔語法撰寫腳本、自動化工具，串接不同語言。**

[![Repository CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

寫一個 `.ox` 檔案，就能用 `oxid` 執行，也能打包成單一 `.oxb` 檔案。發行版執行檔內含直譯器和專案工具；執行 Oxid 程式不需要安裝 Rust。

**目前版本：0.9，實驗階段。** 現有執行環境是以 Rust 撰寫的直譯器。原生編譯、靜態所有權檢查和編譯器自宿主仍待實作。

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

- 用函式、迴圈、管線、陣列、record 和 JSON 撰寫腳本
- 讀寫檔案、啟動外部程序，透過程序轉接器使用 Python、Java 或 Go
- 將模組打包成單一 `.oxb`，鎖定本機路徑或指定 Git commit 的相依套件
- 試作本機 HTTP 服務與 Discord 互動處理邏輯
- 產生 Python、Java、Go、C、C++ 呼叫 Oxid 的轉接程式

從[範例](examples/)、[執行環境 API](docs/API.md)或[跨語言指南](docs/INTEROP.md)開始。外部轉接器仍需要對應的執行環境或工具鏈。

## 使用前先了解

Oxid 0.9 適合實驗與小型工具。`.oxb` 內含序列化 AST，由直譯器執行。Task 採延遲執行，join 時依序執行；網路功能尚無並行排程器或內建 TLS。沿用的 `bootstrap`、`self-host` 指令驗證的是產物序列化往返，並非編譯器重建自身。

長期方向是具備靜態檢查與原生編譯的語言。這些能力、Rust 相容性與原生 AI 訓練，在 0.9 都尚未提供。請參考[路線圖](docs/ROADMAP.md)與[實作現況](docs/architecture/current-baseline.md)。

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
